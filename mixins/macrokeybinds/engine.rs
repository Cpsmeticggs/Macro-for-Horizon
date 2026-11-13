use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
pub enum MacroType { #[default] Simple, Toggle, Repeat, Delayed, CountedRepeat, Alternate }
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
pub enum ActionType { #[default] Message, Command, FillChat, LocalMessage }
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
pub enum Modifier { #[default] None, Shift, Control, Alt }
#[derive(Clone,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
pub struct Action { pub kind:ActionType,pub text:String }
#[derive(Clone,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
pub struct Macro { pub id:String,pub scope:String,pub name:String,pub enabled:bool,pub key:String,pub modifier:Modifier,pub kind:MacroType,pub action:Action,pub second_action:Action,pub delay_ms:u64,pub interval_ms:u64,pub count:u32,pub created:u64 }
#[derive(Clone,Default,Serialize,Deserialize)]
pub struct WheelSlot { pub macro_id:Option<String>,pub icon:Option<String> }
#[derive(Serialize,Deserialize)]
pub struct Wheel {pub hold:bool,pub slots:Vec<WheelSlot>}
impl Default for Wheel {fn default()->Self { Self {hold:false,slots:vec![WheelSlot::default();6]} }}
#[derive(Default,Serialize,Deserialize)]
pub struct Engine {
 #[serde(default)] pub imports:std::collections::HashSet<String>,
 #[serde(skip)] cooldown:HashMap<String,u64>,
 pub macros:Vec<Macro>,pub wheel:Wheel,
 #[serde(skip)] pub server:String,
 #[serde(skip)] jobs:HashMap<String,(u64,u32)>,
 #[serde(skip)] next_b:std::collections::HashSet<String>,
}
impl Engine {
 pub fn stop(&mut self,id:&str){self.jobs.remove(id);self.next_b.remove(id);}
 pub fn stop_all(&mut self){self.jobs.clear();self.cooldown.clear();self.next_b.clear();}
 pub fn running(&self,id:&str)->bool{self.jobs.contains_key(id)}
 pub fn second_next(&self,id:&str)->bool{self.next_b.contains(id)}
 pub fn trigger(&mut self,id:&str,now:u64,held:bool){
    let Some(m)=self.macros.iter().find(|m|m.id==id && m.enabled && (m.scope.is_empty() || m.scope==self.server)) else{return};
    if m.kind==MacroType::Toggle && self.jobs.remove(id).is_some(){return;}
    if matches!(m.kind,MacroType::Delayed|MacroType::CountedRepeat) && self.running(id){return;}
    let count=match m.kind {MacroType::Repeat if held=>u32::MAX,MacroType::Toggle=>u32::MAX,MacroType::CountedRepeat=>m.count.max(1),_=>1};
    let due=match m.kind {
      MacroType::Delayed=>now.saturating_add(m.delay_ms),
      MacroType::Simple|MacroType::Alternate=>now.saturating_add(50),
      MacroType::Repeat|MacroType::Toggle=>now.max(*self.cooldown.get(id).unwrap_or(&0)),
      MacroType::CountedRepeat=>now,
    };
    self.jobs.insert(id.to_owned(),(due,count));
 }
 pub fn key(&mut self,key:&str,pressed:bool,modifier:Modifier,now:u64){
    let matches:Vec<_>=self.macros.iter().filter(|m|m.key==key && !key.is_empty()).map(|m|(m.id.clone(),m.modifier,m.kind)).collect();
    for(id,required,kind)in matches {if pressed && modifier==required{self.trigger(&id,now,true)}else if !pressed && kind==MacroType::Repeat{self.jobs.remove(&id);}}
 }
 pub fn release_held(&mut self,modifier:Option<Modifier>){
    for m in &self.macros{if m.kind==MacroType::Repeat && (modifier.is_none() || (!m.key.starts_with("key.mouse.") && modifier!=Some(m.modifier))){self.jobs.remove(&m.id);}}
 }
 pub fn tick(&mut self,now:u64)->Vec<Action>{
    let mut out=Vec::new();
    for m in &self.macros {
      let Some(&(due,left))=self.jobs.get(&m.id) else{continue};
      if !m.enabled || (!m.scope.is_empty() && m.scope!=self.server){self.jobs.remove(&m.id);continue;}
      if now<due{continue;}
      let second=m.kind==MacroType::Alternate && self.next_b.remove(&m.id);
      out.push(if second{m.second_action.clone()}else{m.action.clone()});
      if m.kind==MacroType::Alternate && !second{self.next_b.insert(m.id.clone());}
      if matches!(m.kind,MacroType::Repeat|MacroType::Toggle){self.cooldown.insert(m.id.clone(),now.saturating_add(m.interval_ms));}
      if left<=1{self.jobs.remove(&m.id);}else{
        let remaining=if matches!(m.kind,MacroType::Toggle|MacroType::Repeat){left}else{left-1};
        self.jobs.insert(m.id.clone(),(now.saturating_add(m.interval_ms),remaining));
      }
    }
    out
 }
}
