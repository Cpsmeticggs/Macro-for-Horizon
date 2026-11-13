use crate::game::{Game, macro_engine::*, macro_screens::{ScreenState,UiEffect}, native_hooks::NativeScreenView,port_macrokeybinds::ID};
use crate::hud::HudBatch;
use std::{sync::{LazyLock,Mutex},time::Instant,path::PathBuf};
#[derive(Default)]
struct State {engine:Engine,screen:ScreenState,root:Option<PathBuf>,close:bool,world:bool,language:Option<String>,load_error:Option<String>}
static STATE:LazyLock<Mutex<State>>=LazyLock::new(Default::default);
static START:LazyLock<Instant>=LazyLock::new(Instant::now);
fn now()->u64{START.elapsed().as_millis() as u64}
fn state()->std::sync::MutexGuard<'static,State>{STATE.lock().unwrap_or_else(|e|e.into_inner())}
fn modifier(bits:u32)->Modifier{if bits&1!=0{Modifier::Shift}else if bits&4!=0{Modifier::Alt}else if bits&2!=0{Modifier::Control}else{Modifier::None}}
fn save(s:&State)->Result<(),String>{
 if let Some(error)=&s.load_error{return Err(error.clone());}
 let dir=s.root.as_ref().ok_or("Missing macro directory")?;
 std::fs::create_dir_all(dir).map_err(|e|e.to_string())?;
 let tmp=dir.join("library.tmp"); let path=dir.join("library.json");
 let data=serde_json::to_vec_pretty(&s.engine).map_err(|e|e.to_string())?;
 std::fs::write(&tmp,data).and_then(|_|std::fs::rename(tmp,path)).map_err(|e|e.to_string())
}
fn load(s:&mut State,g:&Game<'_>){
 if s.root.is_some(){return;}
 let dir=g.native_data_dir(ID);
 s.root=Some(dir.clone());
 match std::fs::read(dir.join("library.json")){
 Ok(bytes)=>match serde_json::from_slice(&bytes){Ok(e)=>s.engine=e,Err(e)=>s.load_error=Some(format!("Cannot read macro library: {e}"))},
 Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{
 for i in 0..8{if let Some(text)=g.native_setting(ID,&format!("slot_{i}")).filter(|x|!x.is_empty()){
 let id=format!("old-slot-{i}");
 s.engine.macros.push(Macro{id:id.clone(),name:format!("Slot {}",i+1),enabled:true,action:Action{kind:ActionType::Message,text},count:1,interval_ms:1000,..Default::default()});
 if i<6{s.engine.wheel.slots[i].macro_id=Some(id);}
 }}
 },Err(e)=>s.load_error=Some(e.to_string())}
 s.engine.wheel.slots.resize_with(6,Default::default);
}
fn effect(s:&mut State,e:UiEffect){
 match e{UiEffect::None=>(),UiEffect::Close=>s.close=true,UiEffect::Save=>if let Err(e)=save(s){s.screen.set_status(e)},UiEffect::Trigger(id)=>{s.engine.trigger(&id,now(),false);s.close=true},UiEffect::Stop(id)=>s.engine.stop(&id),UiEffect::StopAll=>s.engine.stop_all(),UiEffect::Help=>{if let Err(e)=open::that("https://discord.gg/xTqj3ZSeH4"){s.screen.set_status(e.to_string());}}}
}
pub fn open(g:&mut Game<'_>,wheel:bool){
 if g.native_mod_screen()==Some(ID)||!g.native_mod_screen_open(ID){return}
 let mut s=state();load(&mut s,g);s.close=false;
 if wheel{let hold=s.engine.wheel.hold;s.screen.open_wheel(hold)}else{s.screen.open_main()}
 let server=s.engine.server.clone();s.screen.set_server(Some(server));
 if !wheel {s.screen.set_icons(g.ui.registries.items.iter().map(|id|(id.clone(),id.trim_start_matches("minecraft:").replace('_'," "))).collect());}
 if s.language.as_ref()!=Some(&g.settings.language){
 s.screen.set_translations(crate::game::macro_lang::dictionary(&g.settings.language));s.language=Some(g.settings.language.clone());}
 if let Some(e)=s.load_error.clone(){s.screen.set_status(e);}
}
pub fn input(g:&mut Game<'_>,key:&str,pressed:bool){
 if !g.native_window()||!g.native_on(ID){return}
 let wheel=g.native_mod_key(ID,"open_wheel","key.keyboard.n");
 if !pressed && key==wheel && g.native_mod_screen()==Some(ID){release(g,key);return;}
 if !g.native_playing()||g.native_player().is_none()||!g.focused{return}
 if pressed && key==g.native_mod_key(ID,"open_options","key.keyboard.m"){open(g,false);return;}
 if pressed && key==wheel{open(g,true);return;}
 let mut s=state();load(&mut s,g);
 let bits=if key.starts_with("key.mouse."){0}else{super::glfw_modifiers(g.modifiers)};
 s.engine.key(key,pressed,modifier(bits),now());
}
pub fn release(g:&Game<'_>,key:&str){
 if g.native_mod_screen()==Some(ID)&&key==g.native_mod_key(ID,"open_wheel","key.keyboard.n"){
 let mut s=state();let e=s.screen.wheel_release();effect(&mut s,e);}
}
pub fn render(g:&Game<'_>,batch:&mut HudBatch,v:&NativeScreenView<'_>){
 if g.native_mod_screen()!=Some(ID){return}let mut s=state();let State{engine,screen,..}=&mut *s;
 if let Some(prefix)=screen.command_prefix(){
 let cmd=prefix.trim_start_matches('/');let tree=super::chat::chat_tree(&g.remote);let reg=&g.ui.registries;
 let (_,list)=tree.client_suggestions(&tree.parse(cmd,reg),cmd.len(),reg);
 screen.set_suggestions(list.into_iter().take(8).map(|s|format!("/{}",s.apply(cmd))).collect());
 }
 screen.render(g,batch,v,engine);
}
pub fn mouse(g:&Game<'_>,v:&NativeScreenView<'_>,b:i32,c:&mut bool){
 if g.native_mod_screen()!=Some(ID){return}*c=true;let mut s=state();let State{screen,engine,..}=&mut *s;screen.set_modifiers(super::glfw_modifiers(g.modifiers));let e=screen.mouse(v,b,engine);effect(&mut s,e);
}
pub fn key(g:&Game<'_>,k:&str,c:&mut bool){
 if g.native_mod_screen()!=Some(ID){return}*c=true;let mut s=state();let State{screen,engine,..}=&mut *s;let e=screen.key(k,super::glfw_modifiers(g.modifiers),engine);effect(&mut s,e);
}
pub fn character(g:&Game<'_>,ch:char,c:&mut bool){
 if g.native_mod_screen()==Some(ID){*c=true;let mut s=state();s.screen.set_modifiers(super::glfw_modifiers(g.modifiers));s.screen.character(ch);}
}
pub fn scroll(g:&Game<'_>,dy:f64,c:&mut bool){if g.native_mod_screen()==Some(ID){*c=true;state().screen.scroll(dy);}}
pub fn tick(g:&mut Game<'_>){
 if !g.native_window(){return}let enabled=g.native_on(ID);let world=g.native_player().is_some();
 let server=if world && g.remote.is_some() && g.integrated.is_none(){g.server_address().unwrap_or("")}else{""};
 let (close,actions)={let mut s=state();if !enabled||!world{s.engine.stop_all();s.world=false;s.close=false;return;}
 load(&mut s,g);
 if !s.world||s.engine.server!=server{s.engine.stop_all();s.engine.server=server.to_owned();s.world=true;
 if let Ok(config)=Game::native_config_dir(){
   let mut imported=super::macro_import::read_scope(&config,"",&mut s.engine);
   if !server.is_empty(){imported |=super::macro_import::read_scope(&config,server,&mut s.engine);}
   if imported {if let Err(e)=save(&s){s.screen.set_status(e);}}
 }}
 if !g.focused||!g.native_playing(){s.engine.release_held(None)}else{s.engine.release_held(Some(modifier(super::glfw_modifiers(g.modifiers))))}
 (std::mem::take(&mut s.close),s.engine.tick(now()))};
 if close {g.native_mod_screen_close(ID);}
 for action in actions {match action.kind{ActionType::Message=>g.handle_chat_input(&action.text),ActionType::Command=>g.handle_chat_input(&format!("/{}",action.text.trim_start_matches('/'))),ActionType::LocalMessage=>g.native_chat(&action.text),ActionType::FillChat=>g.open_chat(&action.text,false)}}
}
