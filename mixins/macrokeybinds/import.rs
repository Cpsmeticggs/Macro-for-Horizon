use super::macro_engine::*;
use serde_json::Value;
use std::path::Path;
use horizon_client_input::key_names::{MOUSE, KEYBOARD};

fn legacy_sdl(n:i32)->i32{
    (match n {
        0..=7=>return 2001+n,65..=90=>4+n-65,49..=57=>30+n-49,290..=301=>58+n-290,302..=313=>104+n-302,321..=329=>89+n-321,
        32=>44,39=>52,44=>54,45=>45,46=>55,47=>56,48=>39,59=>51,61=>46,91=>47,92=>49,93=>48,96=>53,161=>100,162=>50,
        256=>41,257=>40,258=>43,259=>42,260=>73,261=>76,262=>79,263=>80,264=>81,265=>82,266=>75,267=>78,268=>74,269=>77,
        280=>57,281=>71,282=>83,283=>70,284=>72,320=>98,330=>99,331=>84,332=>85,333=>86,334=>87,335=>88,336=>103,
        340=>225,341=>224,342=>226,343=>227,344=>229,345=>228,346=>230,347=>231,348=>118,_=>0
    }) + 1000
}
fn binding(code:i32,legacy:bool)->String{
    let code=if legacy && (0..1000).contains(&code){legacy_sdl(code)}else{code};
    let (table,offset)=if code>=2000{(MOUSE,2000)}else{(KEYBOARD,1000)};
    table.iter().find(|(n,_)|*n>0 && Some(*n)==code.checked_sub(offset)).map_or("",|(_,name)|*name).to_owned()
}
fn action(kind:&str,text:&str)->Option<Action>{
    Some(Action{kind:match kind{"MESSAGE"=>ActionType::Message,"COMMAND"=>ActionType::Command,"FILL_CHAT"=>ActionType::FillChat,"LOCAL_MESSAGE"=>ActionType::LocalMessage,_=>return None},text:text.to_owned()})
}
fn parse(v:&Value,scope:&str)->Option<Macro>{
    let old=v.get("version").is_none();
    let kind=match v["macroType"].as_str()?{"SIMPLE"=>MacroType::Simple,"DELAYED"=>MacroType::Delayed,"COUNTED_REPEAT"=>MacroType::CountedRepeat,"ALTERNATE"=>MacroType::Alternate,"TOGGLE" if old=>MacroType::Repeat,"REPEAT" if old=>MacroType::Toggle,"TOGGLE"=>MacroType::Toggle,"REPEAT"=>MacroType::Repeat,_=>return None};
    let second_action=if kind==MacroType::Alternate{action(v["secondAction"].as_str()?,v["secondActionText"].as_str()?)?}else{Action::default()};
    let id=v["uuid"].as_str()?;
    Some(Macro{id:if scope.is_empty(){id.to_owned()}else{format!("{scope}/{id}")},scope:scope.to_owned(),name:v["name"].as_str()?.to_owned(),enabled:v["enable"].as_bool().unwrap_or(false),kind,
        key:binding(v["key"].as_i64()?.try_into().ok()?,v.get("inputFormat").is_none()),
        modifier:match v["modifier"].as_str(){Some("SHIFT")=>Modifier::Shift,Some("ALT")=>Modifier::Alt,Some("CONTROL")=>Modifier::Control,_=>Modifier::None},
        action:action(v["action"].as_str()?,v["actionText"].as_str()?)?,second_action,
        delay_ms:v["delayedTime"].as_u64().unwrap_or(0),interval_ms:v[if kind==MacroType::CountedRepeat{"intervalTime"}else{"cooldownTime"}].as_u64().unwrap_or(0),
        count:v["repeatCount"].as_u64().unwrap_or(1).try_into().ok()?,created:v["createdTime"].as_u64().unwrap_or(0)})
}
fn read(path:&Path)->Option<Value>{
    std::fs::read(path).ok().and_then(|bytes|serde_json::from_slice(&bytes).ok())
}
fn server_folder(scope:&str)->String{
    let normalized=scope.trim().to_lowercase();
    let readable:String=normalized.chars().map(|c|if c.is_ascii_alphanumeric()||"._-".contains(c){c}else{'_'}).take(80).collect();
    let mut hash=horizon_protocol::crypto::md5(normalized.as_bytes());hash[6]=(hash[6]&15)|48;hash[8]=(hash[8]&63)|128;
    format!("{}-{}",if readable.is_empty(){"server"}else{&readable},horizon_protocol::crypto::uuid_string(u128::from_be_bytes(hash)))
}
pub fn read_scope(config:&Path,scope:&str,engine:&mut Engine)->bool{
    let mut changed=false;
    let modern=config.join("macrokeybinds");
    for base in [modern.as_path(),config]{
        let mut dirs=Vec::new();
        if scope.is_empty(){dirs.push(base.join("global-macros"));}else{
            let root=base.join("servers-macros");dirs.push(root.join(server_folder(scope)));
            if matches!(Path::new(scope).components().collect::<Vec<_>>().as_slice(),[std::path::Component::Normal(_)]){dirs.push(root.join(scope));}
        }
        for dir in dirs {
            let Ok(files)=dir.read_dir() else{continue};
            for file in files.flatten(){
                let path=file.path();
                if path.extension().and_then(|s|s.to_str())!=Some("json"){continue}
                let Some(m)=read(&path).and_then(|v|parse(&v,scope)) else{continue};
                if engine.imports.insert(format!("macro:{}",m.id)){
                    if !engine.macros.iter().any(|old|old.id==m.id){engine.macros.push(m);}
                    changed=true;
                }
            }
        }
    }
    if scope.is_empty()&&!engine.imports.contains("wheel"){
        if let Some(v)=read(&modern.join("wheel.json")){
            if let Some(slots)=v["macros"].as_array(){
                engine.wheel=Wheel::default();engine.wheel.hold=v["mode"]=="HOLD";
                for (slot,src) in engine.wheel.slots.iter_mut().zip(slots){
                    slot.macro_id=src["macroId"].as_str().map(str::to_owned);
                    slot.icon=src["iconId"].as_str().map(str::to_owned);
                }
                engine.imports.insert("wheel".into());changed=true;
            }
        }
    }
    changed
}
