// Stateful native macro screens; no Game mutation, persistence, or macro execution.
// Adapter contract: pass GLFW modifiers to `key`/`set_modifiers`; feed item and
// command catalogs with the setters. Handle Save immediately and queue effects
// requiring mutable Game to tick. Trigger closes the radial wheel in the adapter.
// `wheel_release` confirms only a Hold wheel; call it on the opening key release.
use crate::game::{macro_engine::{Action, ActionType, Engine, Macro, MacroType, Modifier}, native_hooks::NativeScreenView, Game};
use crate::hud::HudBatch;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiEffect { None, Close, Save, Trigger(String), Stop(String), StopAll, Help }
#[derive(Clone, Debug, PartialEq, Eq)]
enum Page { Main, List(String), Editor, WheelOptions, SelectMacro(usize), Icons(usize), Help }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field { Search, Name, ActionA, ActionB, Delay, Interval, Count }
#[derive(Clone, Debug)]
enum Click {
    Page(Page), Back, Close, New, Edit(String), Delete(String), ConfirmDelete,
    CancelModal, OpenLink, Discard, SaveDraft, Enable(String), Run(String), Stop(String), StopAll,
    Field(Field), Mode(MacroType), ActionType(bool, ActionType), Capture, ClearKey,
    Modifier, DraftEnabled, WheelMode, Select(usize,String), ClearSlot(usize),
    ChooseIcon(String), ApplyIcon(usize), ClearIcon(usize), Suggest(String), ExternalHelp,
}
#[derive(Clone)]
struct Hit { rect: [f32;4], click: Click }
#[derive(Clone)]
struct Draft { value: Macro, delay: String, interval: String, count: String, original: Option<Macro> }
impl Draft {
    fn new(value: Macro, existing: bool) -> Self { Self { delay:value.delay_ms.to_string(), interval:value.interval_ms.to_string(), count:value.count.to_string(), original:existing.then(||value.clone()), value } }
    fn changed(&self) -> bool { self.original.as_ref().map_or(true, |m| m != &self.value || self.delay != m.delay_ms.to_string() || self.interval != m.interval_ms.to_string() || self.count != m.count.to_string()) }
}
#[derive(Clone)]
enum Modal { Delete(String), Discard, Link }

pub struct ScreenState {
    page: Page, return_scope: String, server: Option<String>, search: String,
    draft: Option<Draft>, focus: Option<Field>, caret: usize, select_all: bool,
    capture: bool, modifiers: u32, modal: Option<Modal>, hits: Vec<Hit>,
    offset: f32, max_offset: f32, viewport: [f32;4], status: String,
    wheel: bool, wheel_hold: bool, wheel_selected: Option<String>,
    confirm_focus: bool,
    pending_icon: Option<String>, icon_query: Option<String>, filtered_icons: Vec<usize>,
    wheel_radius: i32, wheel_spans: Vec<(i32,i32,i32,usize)>,
    icons: Vec<(String,String)>, suggestions: Vec<String>, translations: HashMap<String,String>,
}
impl Default for ScreenState {
    fn default() -> Self { Self { page:Page::Main, return_scope:String::new(), server:None, search:String::new(), draft:None, focus:None, caret:0, select_all:false, capture:false, modifiers:0, modal:None, hits:Vec::new(), offset:0., max_offset:0., viewport:[0.;4], status:String::new(), wheel:false, wheel_hold:false, wheel_selected:None, confirm_focus:false, pending_icon:None, icon_query:None, filtered_icons:Vec::new(), wheel_radius:0, wheel_spans:Vec::new(), icons:Vec::new(), suggestions:Vec::new(), translations:HashMap::new() } }
}

impl ScreenState {
    pub fn open_main(&mut self) { self.wheel=false; self.draft=None; self.modal=None; self.go(Page::Main); }
    pub fn open_wheel(&mut self,hold:bool) { self.wheel=true; self.wheel_hold=hold; self.wheel_selected=None; self.hits.clear(); self.status.clear(); self.capture=false; self.modal=None; }
    pub fn is_wheel(&self) -> bool { self.wheel }
    pub fn is_capturing(&self) -> bool { self.capture }
    pub fn set_server(&mut self, server: Option<String>) { self.server=server.filter(|s| !s.is_empty()); }
    pub fn set_modifiers(&mut self, modifiers:u32) { self.modifiers=modifiers; }
    pub fn set_icons(&mut self, icons:Vec<(String,String)>) { self.icons=icons; self.icon_query=None; }
    /// Suggestions are complete replacement strings, including slash for commands.
    pub fn set_suggestions(&mut self, suggestions:Vec<String>) { self.suggestions=suggestions; }
    /// Accepts original Java translation keys (see `translation_key`) or English
    /// source strings for additional native UI labels. Missing values use English.
    pub fn set_translations(&mut self, translations:HashMap<String,String>) { self.translations=translations; }
    pub fn set_status(&mut self, status:impl Into<String>) { self.status=status.into(); }
    pub fn command_prefix(&self) -> Option<String> {
        let d=self.draft.as_ref()?;
        let a=match self.focus? { Field::ActionA=>&d.value.action, Field::ActionB=>&d.value.second_action, _=>return None };
        (a.kind==ActionType::Command || a.text.starts_with('/')).then(||a.text.clone())
    }
    fn tr(&self, value:&str) -> String { self.translations.get(translation_key(value)).or_else(||self.translations.get(value)).cloned().unwrap_or_else(||value.to_owned()) }
    fn go(&mut self, page:Page) { self.page=page; self.search.clear(); self.focus=None; self.capture=false; self.select_all=false; self.offset=0.; self.max_offset=0.; self.status.clear(); self.hits.clear(); self.suggestions.clear(); }
    fn text(&self, game:&Game<'_>, batch:&mut HudBatch, value:&str, at:[f32;2], color:u32, width:f32) {
        let t=self.tr(value); let t=fit(game,&t,width); game.native_text(batch,&t,at,color,true);
    }
    fn button(&mut self, game:&Game<'_>, batch:&mut HudBatch, mouse:[f32;2], r:[f32;4], label:&str, click:Click, enabled:bool, selected:bool) {
        let sprite=if !enabled {"gui/sprites/widget/button_disabled"} else if inside(mouse,r) || selected {"gui/sprites/widget/button_highlighted"} else {"gui/sprites/widget/button"};
        horizon_client::ui::nine_slice(batch,&game.gui.atlas,sprite,r[0],r[1],r[2],r[3],u32::MAX);
        if !label.is_empty(){
            let label=fit(game,&self.tr(label),r[2]-8.);
            let left=r[0]+(r[2]-game.native_text_width(&label))*0.5;
            game.native_text(batch,&label,[left,r[1]+(r[3]-8.)*0.5],if enabled{0xffffff}else{0xa0a0a0},true);
        }
        if enabled { self.hits.push(Hit {rect:r,click}); }
    }
    fn field(&mut self, game:&Game<'_>, batch:&mut HudBatch, r:[f32;4], field:Field, value:&str) {
        let selected=self.focus==Some(field); game.native_fill(batch,r,if selected {0xff34475c} else {0xff101a28});
        if selected {game.native_fill(batch,[r[0],r[1]+r[3]-1.,r[2],1.],0xff70bde7);}
        let end=if selected {self.caret.min(value.len())} else {value.len()};
        let mut shown=value.to_owned(); let mut cursor_x=0.;
        if selected {
            let mut start=0;
            while start<end && game.native_text_width(&value[start..end])>r[2]-17. {start+=value[start..].chars().next().unwrap().len_utf8();}
            shown=value[start..].to_owned(); cursor_x=game.native_text_width(&value[start..end]);
            if self.select_all {game.native_fill(batch,[r[0]+3.,r[1]+3.,game.native_text_width(&shown).min(r[2]-6.),r[3]-6.],0xff496c92);}
        }
        // User-entered strings must never pass through translation lookup.
        game.native_text(batch,&fit(game,&shown,r[2]-10.),[r[0]+5.,r[1]+6.],0xffffff,true);
        if selected {game.native_fill(batch,[r[0]+5.+cursor_x,r[1]+4.,1.,12.],0xffeeeeee);}
        self.hits.push(Hit{rect:r,click:Click::Field(field)});
    }
    fn visible(&self,y:f32,h:f32)->bool {y>=self.viewport[1] && y+h<=self.viewport[1]+self.viewport[3]}
    fn row(&self, row:f32, height:f32)->Option<[f32;4]> { let y=self.viewport[1]+row-self.offset; self.visible(y,height).then_some([self.viewport[0],y,self.viewport[2],height]) }
    fn content_height(&mut self,h:f32) {self.max_offset=(h-self.viewport[3]).max(0.); self.offset=self.offset.clamp(0.,self.max_offset);}

    pub fn render(&mut self, game:&Game<'_>, batch:&mut HudBatch, view:&NativeScreenView<'_>, engine:&Engine) {
        self.hits.clear();
        if self.wheel {self.render_wheel(game,batch,view,engine);return;}
        let [x,top,w,available]=view.rect;
        game.native_fill(batch,view.rect,0x80000000);
        let (width,height)=match self.page {Page::Main=>(246.,235.), Page::Editor=>(412.,382.), Page::Icons(_)=>(422.,370.), _=>(466.,356.)};
        let h=available.min(height);
        let y=top+(available-h)*0.5;
        let pw=(w-12.).min(width); let px=x+(w-pw)/2.;
        game.native_fill(batch,[px,y+5.,pw,h-10.],0xff555555);
        game.native_fill(batch,[px+1.,y+6.,pw-2.,h-12.],0xff222222);
        let title=match &self.page {Page::Main=>"Keybinds Menu",Page::List(s)=>if s.is_empty(){"Global Macros"}else{"Server Macros"},Page::Editor=>if self.draft.as_ref().is_some_and(|d|d.original.is_none()){"Create Macro"}else{"Edit Macro"},Page::WheelOptions=>"Wheel options",Page::SelectMacro(_)=>"Select a macro",Page::Icons(_)=>"Select an icon",Page::Help=>"Macro help"};
        let heading=fit(game,&self.tr(title),pw-24.);
        game.native_text(batch,&heading,[px+(pw-game.native_text_width(&heading))*0.5,y+20.],0xffffff,true);
        self.viewport=[px+10.,y+45.,pw-20.,(h-94.).max(20.)];
        match self.page.clone() {
            Page::Main=>self.render_main(game,batch,view,engine),
            Page::List(scope)=>self.render_list(game,batch,view,engine,&scope,false),
            Page::Editor=>self.render_editor(game,batch,view,engine),
            Page::WheelOptions=>self.render_options(game,batch,view,engine),
            Page::SelectMacro(slot)=>self.render_list(game,batch,view,engine,"",true_slot(slot)),
            Page::Icons(slot)=>self.render_icons(game,batch,view,slot),
            Page::Help=>self.render_help(game,batch,view),
        }
        let footer=y+h-39.;
        let mid=px+pw*0.5;
        let bw=((pw-28.)*0.5).min(100.);
        match self.page {
            Page::Editor | Page::Icons(_) => {
                let (label,action,enabled)=match self.page {
                    Page::Icons(slot)=>(if self.pending_icon.is_some(){"Done"}else{"Remove icon"},Click::ApplyIcon(slot),true),
                    _=>(title,Click::SaveDraft,self.draft.as_ref().is_some_and(|d|validate(d).is_ok())),
                };
                self.button(game,batch,view.mouse,[mid-bw-2.,footer,bw,20.],label,action,enabled,false);
                self.button(game,batch,view.mouse,[mid+2.,footer,bw,20.],"Cancel",Click::Back,true,false);
            }
            _=>self.button(game,batch,view.mouse,[mid-bw,footer,bw*2.,20.],if self.page==Page::Main{"Done"}else{"Back"},Click::Back,true,false),
        }
        if !self.status.is_empty(){self.text(game,batch,&self.status,[px+10.,footer+22.],0xffcc83,pw-20.);}
        if self.max_offset>0. {
            let [vx,vy,vw,vh]=self.viewport;let thumb=(vh*vh/(vh+self.max_offset)).max(10.);
            game.native_fill(batch,[vx+vw+3.,vy,3.,vh],0xff101823);
            game.native_fill(batch,[vx+vw+3.,vy+(vh-thumb)*self.offset/self.max_offset,3.,thumb],0xff6a8ba9);
        }
        if self.capture {
            self.hits.clear();
            game.native_fill(batch,view.rect,0xad000000);
            let width=pw.min(300.)-12.;
            let left=mid-width*0.5;
            let top=y+h*0.5-35.;
            game.native_fill(batch,[left,top,width,70.],0xff666666);
            game.native_fill(batch,[left+1.,top+1.,width-2.,68.],0xff202020);
            let instruction=if self.status.is_empty(){"Esc cancels   /   Delete clears the binding"}else{&self.status};
            for (line,dy,color) in [("Press a key or mouse button",17.,0xffffff),(instruction,42.,0xcccccc)]{
                let line=fit(game,&self.tr(line),width-14.);
                game.native_text(batch,&line,[mid-game.native_text_width(&line)*0.5,top+dy],color,true);
            }
        }
        if self.modal.is_some() {self.render_modal(game,batch,view,engine);}
    }

    fn render_main(&mut self,g:&Game<'_>,b:&mut HudBatch,v:&NativeScreenView<'_>,e:&Engine) {
        let running=e.macros.iter().any(|m|e.running(&m.id));
        let rows=[("Global Macros",Click::Page(Page::List(String::new())),true),
            ("Server Macros",Click::Page(Page::List(self.server.clone().unwrap_or_default())),self.server.is_some()),
            ("Wheel options",Click::Page(Page::WheelOptions),true),("Macro help",Click::Page(Page::Help),true)];
        let mut y=0.;
        for (label,action,enabled) in rows {
            if let Some(r)=self.row(y,20.) {self.button(g,b,v.mouse,r,label,action,enabled,false);}
            y+=32.;
        }
        if running {if let Some(r)=self.row(y,20.){self.button(g,b,v.mouse,r,"Stop all running macros",Click::StopAll,true,false);}y+=32.;}
        self.content_height(y-12.);
    }

    fn render_list(&mut self,g:&Game<'_>,b:&mut HudBatch,v:&NativeScreenView<'_>,e:&Engine,scope:&str,selector:bool) {
        let [x,y,w,_]=self.viewport; let search=self.search.clone();
        self.field(g,b,[x,y,w-94.,23.],Field::Search,&search);
        self.button(g,b,v.mouse,[x+w-88.,y,88.,23.],if selector {"Clear slot"} else {"Create Macro"},if let Page::SelectMacro(slot)=self.page {Click::ClearSlot(slot)}else{Click::New},true,false);
        self.text(g,b,"Search for macros",[x,y+28.],0x93aac4,w);
        let saved=self.viewport; self.viewport[1]+=44.;self.viewport[3]-=44.;
        let needle=self.search.to_lowercase();
        let items:Vec<_>=e.macros.iter().filter(|m|m.scope==scope && (m.name.to_lowercase().contains(&needle)||m.action.text.to_lowercase().contains(&needle)||m.key.to_lowercase().contains(&needle))).collect();
        if items.is_empty() {if let Some(r)=self.row(8.,20.){self.text(g,b,if selector {"No macro available"}else{"No matching macros. Create a macro to begin."},[r[0]+3.,r[1]],0xa4b4c8,r[2]-6.);}}
        for (i,m) in items.iter().enumerate() {if let Some(r)=self.row(i as f32*57.,52.) {
            g.native_fill(b,r,0xff1c2b40);
            let label=format!("{}{}{}",m.name,if e.running(&m.id){"  [Running]"}else{""},if e.second_next(&m.id){"  [B next]"}else{""});
            self.text(g,b,&label,[r[0]+5.,r[1]+5.],if m.enabled{0xffffff}else{0x8993a2},r[2]-10.);
            if selector {
                let slot=if let Page::SelectMacro(slot)=self.page {slot}else{0};
                self.text(g,b,&format!("{} | {}",mode_label(m.kind),binding_label(m)),[r[0]+5.,r[1]+32.],0xb9c9dc,r[2]-80.);
                self.button(g,b,v.mouse,[r[0]+r[2]-73.,r[1]+25.,68.,22.],"Select",Click::Select(slot,m.id.clone()),true,false);
            }else{
                // Each row has its own controls; lists are unrestricted and scrollable.
                let bw=((r[2]-12.)/5.).min(93.);let labels=[if m.enabled{"Enabled"}else{"Disabled"},"Edit Macro","Delete",if e.running(&m.id){"Stop"}else{"Run"},mode_label(m.kind)];
                let acts=[Click::Enable(m.id.clone()),Click::Edit(m.id.clone()),Click::Delete(m.id.clone()),if e.running(&m.id){Click::Stop(m.id.clone())}else{Click::Run(m.id.clone())},Click::Edit(m.id.clone())];
                for j in 0..5 {self.button(g,b,v.mouse,[r[0]+5.+j as f32*(bw+1.),r[1]+25.,bw,22.],labels[j],acts[j].clone(),j!=3 || m.enabled || e.running(&m.id),false);}
            }
        }}
        self.content_height(items.len() as f32*57.); self.viewport=saved;
    }

    fn render_editor(&mut self,g:&Game<'_>,b:&mut HudBatch,v:&NativeScreenView<'_>,e:&Engine) {
        let Some(d)=self.draft.clone() else {return};
        let mut row=0.;
        self.edit_field(g,b,&mut row,"Macro name",Field::Name,&d.value.name);
        if let Some(r)=self.row(row,24.) {self.button(g,b,v.mouse,[r[0],r[1],r[2]/2.-3.,24.],if d.value.enabled{"Enabled"}else{"Disabled"},Click::DraftEnabled,true,d.value.enabled);self.text(g,b,&format!("Scope: {}",if d.value.scope.is_empty(){"Global"}else{&d.value.scope}),[r[0]+r[2]/2.+3.,r[1]+7.],0xb9c9dc,r[2]/2.-6.);} row+=31.;
        if let Some(r)=self.row(row,14.) {self.text(g,b,"Macro type",[r[0],r[1]],0xc1d4e9,r[2]);} row+=17.;
        let modes=[MacroType::Simple,MacroType::Toggle,MacroType::Repeat,MacroType::Delayed,MacroType::CountedRepeat,MacroType::Alternate];
        let next=modes[(modes.iter().position(|m|*m==d.value.kind).unwrap_or(0)+1)%modes.len()];
        if let Some(r)=self.row(row,20.){self.button(g,b,v.mouse,r,mode_label(d.value.kind),Click::Mode(next),true,false);} row+=26.;
        self.action_editor(g,b,v,&mut row,false,&d.value.action);
        if d.value.kind==MacroType::Alternate {self.action_editor(g,b,v,&mut row,true,&d.value.second_action);}
        if d.value.kind==MacroType::Delayed{self.edit_field(g,b,&mut row,"Delay (ms)",Field::Delay,&d.delay);}
        if matches!(d.value.kind,MacroType::Toggle|MacroType::Repeat|MacroType::CountedRepeat) {self.edit_field(g,b,&mut row,if d.value.kind==MacroType::CountedRepeat{"Interval (ms)"}else{"Cooldown (ms)"},Field::Interval,&d.interval);}
        if d.value.kind==MacroType::CountedRepeat {self.edit_field(g,b,&mut row,"Count",Field::Count,&d.count);}
        if let Some(r)=self.row(row,14.) {self.text(g,b,"Key binding",[r[0],r[1]],0xc1d4e9,r[2]);}row+=17.;
        if let Some(r)=self.row(row,24.) {
            let bw=(r[2]-8.)/3.;self.button(g,b,v.mouse,[r[0],r[1],bw,24.],&binding_label(&d.value),Click::Capture,true,self.capture);
            self.button(g,b,v.mouse,[r[0]+bw+4.,r[1],bw,24.],modifier_label(d.value.modifier),Click::Modifier,true,false);
            self.button(g,b,v.mouse,[r[0]+2.*(bw+4.),r[1],bw,24.],"Unbind",Click::ClearKey,true,false);
        }row+=30.;
        let conflicts:Vec<_>=e.macros.iter().filter(|m|m.id!=d.value.id && m.enabled && !d.value.key.is_empty() && m.key==d.value.key && m.modifier==d.value.modifier && (m.scope==d.value.scope || m.scope.is_empty() || d.value.scope.is_empty())).map(|m|m.name.clone()).collect();
        if !conflicts.is_empty() {row=self.paragraph(g,b,row,&format!("Key conflict (allowed): {}",conflicts.join(", ")),0xffbd6b);}
        if let Err(err)=validate(&d) {row=self.paragraph(g,b,row,&err,0xff9c91);}

        self.content_height(row+5.);
    }
    fn edit_field(&mut self,g:&Game<'_>,b:&mut HudBatch,row:&mut f32,label:&str,f:Field,value:&str) {
        if let Some(r)=self.row(*row,14.){self.text(g,b,label,[r[0],r[1]],0xc1d4e9,r[2]);}*row+=17.;
        if let Some(r)=self.row(*row,24.){self.field(g,b,r,f,value);}*row+=31.;
    }
    fn action_editor(&mut self,g:&Game<'_>,b:&mut HudBatch,v:&NativeScreenView<'_>,row:&mut f32,second:bool,a:&Action) {
        if let Some(r)=self.row(*row,14.){self.text(g,b,if second{"Action B"}else{"Action A"},[r[0],r[1]],0xc1d4e9,r[2]);}*row+=17.;
        let kinds=[ActionType::Message,ActionType::Command,ActionType::FillChat,ActionType::LocalMessage];
        let next=kinds[(kinds.iter().position(|k|*k==a.kind).unwrap_or(0)+1)%kinds.len()];
        if let Some(r)=self.row(*row,22.){
            let bw=(r[2]*0.44).min(150.);
            self.button(g,b,v.mouse,[r[0],r[1],bw,22.],action_label(a.kind),Click::ActionType(second,next),true,false);
            self.field(g,b,[r[0]+bw+4.,r[1],r[2]-bw-4.,22.],if second{Field::ActionB}else{Field::ActionA},&a.text);
        }
        *row+=28.;
        if self.focus==Some(if second{Field::ActionB}else{Field::ActionA}) && self.command_prefix().is_some() {
            for s in self.suggestions.clone().into_iter().take(5) {if let Some(r)=self.row(*row,21.){self.button(g,b,v.mouse,r,&s,Click::Suggest(s.clone()),true,false);}*row+=23.;}
        }
    }
    fn paragraph(&self,g:&Game<'_>,b:&mut HudBatch,mut row:f32,text:&str,color:u32)->f32 {
        for line in wrap(g,&self.tr(text),self.viewport[2]) {if let Some(r)=self.row(row,12.){g.native_text(b,&line,[r[0],r[1]],color,true);}row+=14.;}row+5.
    }

    fn render_options(&mut self,g:&Game<'_>,b:&mut HudBatch,v:&NativeScreenView<'_>,e:&Engine) {
        if let Some(r)=self.row(0.,24.) {self.button(g,b,v.mouse,r,if e.wheel.hold{"Mode: Hold"}else{"Mode: Click"},Click::WheelMode,true,false);}
        for i in 0..6 {if let Some(r)=self.row(33.+i as f32*65.,59.) {
            let slot=e.wheel.slots.get(i);let m=slot.and_then(|s|s.macro_id.as_ref()).and_then(|id|e.macros.iter().find(|m|&m.id==id));
            g.native_fill(b,r,0xff1c2b40);
            if let Some(icon)=slot.and_then(|s|s.icon.as_ref()) {g.native_item(b,icon,[r[0]+5.,r[1]+5.],18.);}
            let label=format!("{}: {}",i+1,m.map(|m|m.name.as_str()).unwrap_or("Empty"));self.text(g,b,&label,[r[0]+28.,r[1]+8.],0xffffff,r[2]-33.);
            let bw=(r[2]-16.)/4.;let buttons=[("Select a macro",Click::Page(Page::SelectMacro(i))),("Remove macro",Click::ClearSlot(i)),("Change icon",Click::Page(Page::Icons(i))),("Remove icon",Click::ClearIcon(i))];
            for (j,(label,c)) in buttons.into_iter().enumerate(){self.button(g,b,v.mouse,[r[0]+4.+j as f32*(bw+3.),r[1]+31.,bw,23.],label,c,true,false);}
        }}
        self.content_height(33.+6.*65.);
    }
    fn render_icons(&mut self,g:&Game<'_>,b:&mut HudBatch,v:&NativeScreenView<'_>,_slot:usize) {
        let [x,y,w,_]=self.viewport;let search=self.search.clone();self.field(g,b,[x,y,w,23.],Field::Search,&search);
        
        let saved=self.viewport;self.viewport[1]+=44.;self.viewport[3]-=44.;
        if self.icon_query.as_ref()!=Some(&self.search){
            let query=self.search.to_lowercase();
            self.filtered_icons=self.icons.iter().enumerate().filter(|(_, (id,label))|id.contains(&query)||label.contains(&query)).map(|(i,_)|i).collect();
            self.icon_query=Some(self.search.clone());
        }
        let count=self.filtered_icons.len();
        let cols=((w/28.).floor() as usize).clamp(1,12);
        let step=w/cols as f32;
        let first=(self.offset/26.).floor() as usize*cols;
        let end=(first+((self.viewport[3]/26.).ceil() as usize+1)*cols).min(count);
        let mut preview=self.pending_icon.clone();
        for i in first..end {
            if let Some(r)=self.row((i/cols) as f32*26.,24.) {
                let (id,name)=&self.icons[self.filtered_icons[i]];
                let (id,name)=(id.clone(),name.clone());
                let rect=[r[0]+(i%cols) as f32*step,r[1],step-2.,24.];
                let selected=self.pending_icon.as_ref()==Some(&id);
                if inside(v.mouse,rect){preview=Some(name);}
                self.button(g,b,v.mouse,rect,"",Click::ChooseIcon(id.clone()),true,selected);
                g.native_item(b,&id,[rect[0]+(rect[2]-16.)*0.5,rect[1]+4.],16.);
            }
        }
        if count==0{self.text(g,b,"No matching items",[x,y+46.],0xbcbcbc,w);}
        if let Some(label)=preview {self.text(g,b,&label,[x,y+28.],0xffffff,w);}
        self.content_height(count.div_ceil(cols) as f32*26.);self.viewport=saved;
    }
    fn render_help(&mut self,g:&Game<'_>,b:&mut HudBatch,v:&NativeScreenView<'_>) {
        let mut row=0.;
        for text in ["Macros run only in a loaded world. Global macros work everywhere; server macros apply only to their saved server.",mode_help(MacroType::Simple),mode_help(MacroType::Toggle),mode_help(MacroType::Repeat),mode_help(MacroType::Delayed),mode_help(MacroType::CountedRepeat),mode_help(MacroType::Alternate),"Send Message sends chat (a leading / executes a command). Execute Command sends a server or client command. Fill Chat opens editable text without sending. Display Local Message is visible only to you.","Cooldown and delay use milliseconds (1000 ms = 1 second). Interval controls repeating actions. Count must be a positive whole number.","Click a key binding, then press a key or mouse button. Escape cancels capture; Delete clears the binding. Choose None, Shift, Control or Alt. Conflicting enabled bindings show a warning and may all activate.","Wheel options contain six slots referencing global macros. Choose any registered item as an icon. Click mode runs the selected slice with a click. Hold mode runs it when you release the wheel key. Move to the center to cancel.","Use the search boxes to filter macros and item icons. Editor fields support Home, End, Left, Right, Backspace, Delete and Ctrl+A. Tab accepts the first command suggestion when available. Scroll to reach every option.","Save applies an edited macro. Back or Close asks before discarding changes. Deletion requires confirmation. Stop all cancels every running macro."] {row=self.paragraph(g,b,row,text,0xc2d0e2);row+=8.;}
        if let Some(r)=self.row(row,24.){self.button(g,b,v.mouse,r,"Community help",Click::ExternalHelp,true,false);}self.content_height(row+30.);
    }
    fn render_modal(&mut self,g:&Game<'_>,b:&mut HudBatch,v:&NativeScreenView<'_>,e:&Engine) {
        self.hits.clear();
        let [x,y,w,h]=v.rect;
        let width=(w-24.).min(320.);
        let left=x+(w-width)*0.5;
        let top=y+h*0.5-48.;
        g.native_fill(b,v.rect,0xad000000);
        g.native_fill(b,[left,top,width,96.],0xff666666);
        g.native_fill(b,[left+1.,top+1.,width-2.,94.],0xff202020);
        let (title,detail,yes,action)=match self.modal.as_ref().unwrap(){
            Modal::Delete(id)=>("Delete this macro?",e.macros.iter().find(|m|&m.id==id).map(|m|m.name.clone()).unwrap_or_default(),"Delete this macro",Click::ConfirmDelete),
            Modal::Discard=>("Discard unsaved changes?",self.tr("Your changes will be lost."),"Discard",Click::Discard),
            Modal::Link=>("Open this link?","https://discord.gg/xTqj3ZSeH4".to_owned(),"Open link",Click::OpenLink),
        };
        for (label,dy,color) in [(self.tr(title),16.,0xffffff),(detail,37.,0xcccccc)] {
            let label=fit(g,&label,width-20.);
            g.native_text(b,&label,[left+(width-g.native_text_width(&label))*0.5,top+dy],color,true);
        }
        let bw=(width-26.)*0.5;
        self.button(g,b,v.mouse,[left+10.,top+64.,bw,20.],yes,action,true,self.confirm_focus);
        self.button(g,b,v.mouse,[left+16.+bw,top+64.,bw,20.],"Cancel",Click::CancelModal,true,!self.confirm_focus);
    }
    fn render_wheel(&mut self,g:&Game<'_>,b:&mut HudBatch,v:&NativeScreenView<'_>,e:&Engine) {
        let [x,y,w,h]=v.rect;let center=[x+w/2.,y+h/2.];let radius=(w.min(h)*0.40).min(140.).max(40.);let inner=radius*0.30;
        g.native_fill(b,v.rect,0x9e0b111c);self.wheel_hold=e.wheel.hold;self.wheel_selected=None;
        let selected=wheel_sector(v.mouse,center,inner,radius*1.23);
        // Cache contiguous horizontal runs at the wheel's GUI size.
        let pixels=radius as i32;
        if self.wheel_radius!=pixels {
            self.wheel_radius=pixels;
            self.wheel_spans.clear();
            for y in -pixels..pixels {
                let mut start=-pixels;
                let mut previous=None;
                for x in -pixels..=pixels {
                    let sector=if x==pixels{None}else{wheel_sector([x as f32+0.5,y as f32+0.5],[0.,0.],pixels as f32*0.30,pixels as f32)};
                    if sector!=previous {
                        if let Some(index)=previous{self.wheel_spans.push((start,y,x-start,index));}
                        start=x; previous=sector;
                    }
                }
            }
        }
        for &(x,y,width,index) in &self.wheel_spans{
            let color=if selected==Some(index){0xe84c779e}else{0xd527384f};
            g.native_fill(b,[center[0]+x as f32,center[1]+y as f32,width as f32,1.],color);
        }
        for i in 0..6 {let slot=e.wheel.slots.get(i);let m=slot.and_then(|s|s.macro_id.as_ref()).and_then(|id|e.macros.iter().find(|m|&m.id==id && m.scope.is_empty()));
            let angle=i as f32*std::f32::consts::TAU/6.-std::f32::consts::FRAC_PI_2;
            let at=[center[0]+angle.cos()*radius*0.67,center[1]+angle.sin()*radius*0.67];
            if let Some(icon)=slot.and_then(|s|s.icon.as_ref()){g.native_item(b,icon,[at[0]-9.,at[1]-17.],18.);}
            let label=m.map(|m|m.name.as_str()).unwrap_or("Empty");let label=fit(g,&self.tr(label),radius*0.85);let tw=g.native_text_width(&label);g.native_text(b,&label,[at[0]-tw/2.,at[1]+5.],if m.map_or(false,|m|m.enabled){0xffffff}else{0x858f9e},true);
            if selected==Some(i){if let Some(m)=m{if m.enabled{self.wheel_selected=Some(m.id.clone());}else{self.status=self.tr("Disabled");}}else{self.status=self.tr("Empty");}}
        }
        if selected.is_none(){self.status=self.tr("Cancel");}
        let center_label=if self.wheel_selected.is_some(){if e.wheel.hold{"Release"}else{"Click"}}else{&self.status};
        let tw=g.native_text_width(center_label);g.native_text(b,center_label,[center[0]-tw/2.,center[1]-4.],0xffffff,true);
        self.text(g,b,if e.wheel.hold{"Release the wheel key to run; center cancels"}else{"Click a slice to run; Esc closes"},[x+12.,y+h-24.],0xe0ebf9,w-24.);
    }

    pub fn mouse(&mut self,view:&NativeScreenView<'_>,button:i32,engine:&mut Engine)->UiEffect {
        if self.capture {
            let key=match button {0=>"key.mouse.left".to_owned(),1=>"key.mouse.right".to_owned(),2=>"key.mouse.middle".to_owned(),n=>format!("key.mouse.{}",n+1)};
            self.capture_binding(key);return UiEffect::None;
        }
        if self.wheel {
            if button!=0{return UiEffect::None;}
            // Recompute hover on click even if no frame has rendered since movement.
            let [x,y,w,h]=view.rect;let radius=(w.min(h)*0.40).min(140.).max(40.);
            self.wheel_selected=wheel_sector(view.mouse,[x+w/2.,y+h/2.],radius*0.30,radius*1.23).and_then(|i|engine.wheel.slots.get(i)).and_then(|s|s.macro_id.as_ref()).and_then(|id|engine.macros.iter().find(|m|&m.id==id&&m.enabled&&m.scope.is_empty())).map(|m|m.id.clone());
            if engine.wheel.hold{return UiEffect::None;}
            return self.finish_wheel();
        }
        if button!=0{return UiEffect::None;}
        let click=self.hits.iter().rev().find(|h|inside(view.mouse,h.rect)).map(|h|h.click.clone());
        if let Some(c)=click {self.activate(c,engine)}else{self.focus=None;UiEffect::None}
    }
    fn activate(&mut self,c:Click,e:&mut Engine)->UiEffect {
        match c {
            Click::Page(page)=>{if let Page::Icons(i)=&page {self.pending_icon=e.wheel.slots.get(*i).and_then(|s|s.icon.clone());}if self.page==Page::Editor && self.draft.as_ref().map_or(false,|d|d.changed()){self.status=self.tr("Save or cancel your edit before navigating.");}else{self.go(page);}},
            Click::Back=>return self.back(),Click::Close=>{if self.page==Page::Editor&&self.draft.as_ref().map_or(false,|d|d.changed()){self.hits.clear();self.confirm_focus=false;self.modal=Some(Modal::Discard);}else{return UiEffect::Close;}},
            Click::New=>{let scope=if let Page::List(s)=&self.page{s.clone()}else{String::new()};self.return_scope=scope.clone();let mut m=Macro::default();m.scope=scope;m.enabled=true;m.count=1;m.interval_ms=1000;self.draft=Some(Draft::new(m,false));self.go(Page::Editor);self.focus_field(Field::Name);},
            Click::Edit(id)=>{if let Some(m)=e.macros.iter().find(|m|m.id==id){self.return_scope=m.scope.clone();self.draft=Some(Draft::new(m.clone(),true));self.go(Page::Editor);}},
            Click::Delete(id)=>{self.hits.clear();self.confirm_focus=false;self.modal=Some(Modal::Delete(id));},
            Click::ConfirmDelete=>{if let Some(Modal::Delete(id))=self.modal.take(){e.stop(&id);e.macros.retain(|m|m.id!=id);for slot in &mut e.wheel.slots{if slot.macro_id.as_ref()==Some(&id){slot.macro_id=None;}}self.hits.clear();return UiEffect::Save;}},
            Click::CancelModal=>{self.hits.clear();self.modal=None;},
            Click::Discard=>{self.modal=None;self.draft=None;self.go(Page::List(self.return_scope.clone()));},
            Click::SaveDraft=>return self.save_draft(e),
            Click::Enable(id)=>{e.stop(&id);if let Some(m)=e.macros.iter_mut().find(|m|m.id==id){m.enabled=!m.enabled;}return UiEffect::Save;},
            Click::Run(id)=>return UiEffect::Trigger(id),Click::Stop(id)=>return UiEffect::Stop(id),Click::StopAll=>return UiEffect::StopAll,
            Click::Field(f)=>self.focus_field(f),
            Click::Mode(mode)=>{if let Some(d)=self.draft.as_mut(){d.value.kind=mode;}self.hits.clear();},
            Click::ActionType(second,kind)=>{if let Some(d)=self.draft.as_mut(){if second{d.value.second_action.kind=kind;}else{d.value.action.kind=kind;}}self.suggestions.clear();},
            Click::Capture=>{self.status.clear();self.capture=true;self.focus=None;},Click::ClearKey=>{if let Some(d)=self.draft.as_mut(){d.value.key.clear();d.value.modifier=Modifier::None;}},
            Click::Modifier=>{if let Some(d)=self.draft.as_mut(){d.value.modifier=match d.value.modifier{Modifier::None=>Modifier::Shift,Modifier::Shift=>Modifier::Control,Modifier::Control=>Modifier::Alt,Modifier::Alt=>Modifier::None};}},
            Click::DraftEnabled=>{if let Some(d)=self.draft.as_mut(){d.value.enabled=!d.value.enabled;}},
            Click::WheelMode=>{e.wheel.hold=!e.wheel.hold;return UiEffect::Save;},
            Click::Select(i,id)=>{ensure_slots(e);e.wheel.slots[i].macro_id=Some(id);self.go(Page::WheelOptions);return UiEffect::Save;},
            Click::ClearSlot(i)=>{ensure_slots(e);e.wheel.slots[i].macro_id=None;if matches!(self.page,Page::SelectMacro(_)){self.go(Page::WheelOptions);}return UiEffect::Save;},
            Click::ChooseIcon(id)=>{self.pending_icon=Some(id);},
            Click::ApplyIcon(i)=>{ensure_slots(e);e.wheel.slots[i].icon=self.pending_icon.take();self.go(Page::WheelOptions);return UiEffect::Save;},
            Click::ClearIcon(i)=>{ensure_slots(e);e.wheel.slots[i].icon=None;return UiEffect::Save;},
            Click::Suggest(text)=>{if let Some(value)=self.focused_mut(){*value=text;}self.caret=self.focused().map_or(0,|s|s.len());self.select_all=false;self.suggestions.clear();},
            Click::ExternalHelp=>{self.hits.clear();self.confirm_focus=false;self.modal=Some(Modal::Link);},
            Click::OpenLink=>{self.modal=None;self.hits.clear();return UiEffect::Help;},
        }UiEffect::None
    }
    fn save_draft(&mut self,e:&mut Engine)->UiEffect {
        let Some(d)=self.draft.as_ref()else{return UiEffect::None};
        if let Err(err)=validate(d){self.status=err;return UiEffect::None;}
        let mut m=d.value.clone();m.name=m.name.trim().to_owned();if m.kind==MacroType::Delayed{m.delay_ms=d.delay.parse().unwrap();}
        if matches!(m.kind,MacroType::Toggle|MacroType::Repeat|MacroType::CountedRepeat){m.interval_ms=d.interval.parse().unwrap();}
        if m.kind==MacroType::CountedRepeat{m.count=d.count.parse().unwrap();}
        if d.original.is_none(){let mut n=e.macros.len() as u64+1;loop{let id=format!("native-ui-{n}");if !e.macros.iter().any(|m|m.id==id){m.id=id;break;}n+=1;}m.created=e.macros.iter().map(|m|m.created).max().unwrap_or(0).saturating_add(1);}
        e.stop(&m.id);if let Some(old)=e.macros.iter_mut().find(|old|old.id==m.id){*old=m;}else{e.macros.push(m);}
        self.draft=None;self.go(Page::List(self.return_scope.clone()));UiEffect::Save
    }
    fn back(&mut self)->UiEffect {
        if self.modal.is_some(){self.hits.clear();self.modal=None;return UiEffect::None;}
        if self.capture{self.capture=false;return UiEffect::None;}
        if self.wheel{return UiEffect::Close;}
        match self.page {
            Page::Main=>return UiEffect::Close,
            Page::Editor=>{if self.draft.as_ref().map_or(false,|d|d.changed()){self.hits.clear();self.confirm_focus=false;self.modal=Some(Modal::Discard);}else{self.draft=None;self.go(Page::List(self.return_scope.clone()));}},
            Page::SelectMacro(_)|Page::Icons(_)=>self.go(Page::WheelOptions),
            _=>self.go(Page::Main),
        }UiEffect::None
    }
    fn capture_binding(&mut self,key:String) {
        // Original format supports one modifier. Refuse lossy multi-modifier binds.
        let bits=if key.starts_with("key.mouse."){0}else{self.modifiers&15};if bits.count_ones()>1 || bits&8!=0{self.status=self.tr("Choose at most one of Shift, Control or Alt.");return;}
        if let Some(d)=self.draft.as_mut(){d.value.key=key;d.value.modifier=match bits{1=>Modifier::Shift,2=>Modifier::Control,4=>Modifier::Alt,_=>Modifier::None};}
        self.capture=false;self.status.clear();
    }
    pub fn key(&mut self,key:&str,modifiers:u32,engine:&mut Engine)->UiEffect {
        self.modifiers=modifiers;let k=key.strip_prefix("key.keyboard.").unwrap_or(key);
        if k=="escape"{return self.back();}
        if self.capture {
            if k=="delete"||k=="backspace"{self.capture=false;return self.activate(Click::ClearKey,engine);}
            if matches!(k,"left.shift"|"right.shift"|"left.control"|"right.control"|"left.alt"|"right.alt"|"left.win"|"right.win"){return UiEffect::None;}
            self.capture_binding(key.to_owned());return UiEffect::None;
        }
        if let Some(modal)=&self.modal {
            match k {
                "tab"|"left"|"right"=>self.confirm_focus=!self.confirm_focus,
                "enter"|"keypad.enter"|"space"=>{
                    let action=if !self.confirm_focus{Click::CancelModal}else{match modal{Modal::Delete(_)=>Click::ConfirmDelete,Modal::Discard=>Click::Discard,Modal::Link=>Click::OpenLink}};
                    return self.activate(action,engine);
                }
                _=>{}
            }
            return UiEffect::None;
        }
        if self.wheel{return UiEffect::None;}
        if k=="s"&&modifiers&2!=0&&self.page==Page::Editor{return self.save_draft(engine);}
        if k=="tab" {
            if self.command_prefix().is_some(){if let Some(s)=self.suggestions.first().cloned(){return self.activate(Click::Suggest(s),engine);}}
            let fields=if self.page==Page::Editor {let d=self.draft.as_ref().unwrap();let mut f=vec![Field::Name,Field::ActionA];if d.value.kind==MacroType::Alternate{f.push(Field::ActionB);}if d.value.kind==MacroType::Delayed{f.push(Field::Delay);}if matches!(d.value.kind,MacroType::Toggle|MacroType::Repeat|MacroType::CountedRepeat){f.push(Field::Interval);}if d.value.kind==MacroType::CountedRepeat{f.push(Field::Count);}f}else{vec![Field::Search]};
            let at=self.focus.and_then(|f|fields.iter().position(|x|*x==f));let next=if modifiers&1!=0{at.map_or(fields.len()-1,|i|(i+fields.len()-1)%fields.len())}else{at.map_or(0,|i|(i+1)%fields.len())};self.focus_field(fields[next]);return UiEffect::None;
        }
        if k=="page.down"{return self.scroll(-5.);}if k=="page.up"{return self.scroll(5.);}
        let Some(text)=self.focused().cloned()else{return UiEffect::None};
        let pos=self.caret.min(text.len());
        match k {
            "a" if modifiers&2!=0=>{self.select_all=true;self.caret=text.len();},
            "home"=>{self.caret=0;self.select_all=false;},"end"=>{self.caret=text.len();self.select_all=false;},
            "left"=>{self.caret=previous_boundary(&text,pos);self.select_all=false;},"right"=>{self.caret=next_boundary(&text,pos);self.select_all=false;},
            "backspace"|"delete"=>{let (start,end)=if self.select_all{(0,text.len())}else if k=="backspace"{(previous_boundary(&text,pos),pos)}else{(pos,next_boundary(&text,pos))};if let Some(s)=self.focused_mut(){s.replace_range(start..end,"");}self.caret=start;self.select_all=false;self.suggestions.clear();if self.focus==Some(Field::Search){self.offset=0.;}},
            "enter"|"kp.enter"=>{self.focus=None;self.select_all=false;},_=>{}
        }UiEffect::None
    }
    pub fn character(&mut self,c:char)->UiEffect {
        if c.is_control()||self.capture||self.modal.is_some()||self.wheel||self.modifiers&6!=0{return UiEffect::None;}
        let numeric=matches!(self.focus,Some(Field::Delay|Field::Interval|Field::Count));if numeric&&!c.is_ascii_digit(){return UiEffect::None;}
        let Some(value)=self.focused()else{return UiEffect::None};let limit=if numeric{20}else if self.focus==Some(Field::Name){128}else{4096};if value.chars().count()>=limit&&!self.select_all{return UiEffect::None;}
        let pos=if self.select_all{0}else{self.caret.min(value.len())};let all=self.select_all;
        if let Some(s)=self.focused_mut(){if all{s.clear();}s.insert(pos,c);}self.caret=pos+c.len_utf8();self.select_all=false;self.suggestions.clear();if self.focus==Some(Field::Search){self.offset=0.;}UiEffect::None
    }
    pub fn scroll(&mut self,delta:f64)->UiEffect {if !self.wheel&&self.modal.is_none()&&!self.capture&&delta.is_finite(){self.offset=(self.offset-delta as f32*30.).clamp(0.,self.max_offset);self.hits.clear();}UiEffect::None}
    pub fn wheel_release(&mut self)->UiEffect {if self.wheel&&self.wheel_hold {self.finish_wheel()}else{UiEffect::None}}
    fn finish_wheel(&mut self)->UiEffect {self.wheel=false;self.wheel_selected.take().map(UiEffect::Trigger).unwrap_or(UiEffect::Close)}
    fn focus_field(&mut self,f:Field) {self.focus=Some(f);self.caret=self.focused().map_or(0,|s|s.len());self.select_all=false;self.suggestions.clear();}
    fn focused(&self)->Option<&String> {match self.focus? {Field::Search=>Some(&self.search),f=>{let d=self.draft.as_ref()?;Some(match f{Field::Name=>&d.value.name,Field::ActionA=>&d.value.action.text,Field::ActionB=>&d.value.second_action.text,Field::Delay=>&d.delay,Field::Interval=>&d.interval,Field::Count=>&d.count,_=>unreachable!()})}}}
    fn focused_mut(&mut self)->Option<&mut String> {match self.focus? {Field::Search=>Some(&mut self.search),f=>{let d=self.draft.as_mut()?;Some(match f{Field::Name=>&mut d.value.name,Field::ActionA=>&mut d.value.action.text,Field::ActionB=>&mut d.value.second_action.text,Field::Delay=>&mut d.delay,Field::Interval=>&mut d.interval,Field::Count=>&mut d.count,_=>unreachable!()})}}}
}

fn true_slot(_:usize)->bool{true}
fn ensure_slots(e:&mut Engine){e.wheel.slots.resize_with(6,Default::default);}
fn inside(p:[f32;2],r:[f32;4])->bool{p[0]>=r[0]&&p[1]>=r[1]&&p[0]<r[0]+r[2]&&p[1]<r[1]+r[3]}
fn previous_boundary(s:&str,pos:usize)->usize{s[..pos].char_indices().last().map_or(0,|(i,_)|i)}
fn next_boundary(s:&str,pos:usize)->usize{s[pos..].chars().next().map_or(pos,|c|pos+c.len_utf8())}
fn fit(g:&Game<'_>,s:&str,width:f32)->String{if g.native_text_width(s)<=width{return s.to_owned();}let mut out=String::new();for c in s.chars(){if g.native_text_width(&format!("{out}{c}..."))>width.max(0.){break;}out.push(c);}out.push_str("...");out}
fn wrap(g:&Game<'_>,s:&str,width:f32)->Vec<String>{let mut lines=Vec::new();let mut line=String::new();for word in s.split_whitespace(){let next=if line.is_empty(){word.to_owned()}else{format!("{line} {word}")};if g.native_text_width(&next)>width&&!line.is_empty(){lines.push(line);line=word.to_owned();}else{line=next;}}if !line.is_empty(){lines.push(line);}lines}
fn mode_label(m:MacroType)->&'static str{match m{MacroType::Simple=>"Simple",MacroType::Toggle=>"Toggle",MacroType::Repeat=>"Repeat",MacroType::Delayed=>"Delayed",MacroType::CountedRepeat=>"Counted Repeat",MacroType::Alternate=>"Alternate"}}
fn mode_help(m:MacroType)->&'static str{match m{MacroType::Simple=>"Simple: execute once each time the key is pressed.",MacroType::Toggle=>"Toggle: press once to repeat; press again to stop.",MacroType::Repeat=>"Repeat: execute repeatedly while the key is held.",MacroType::Delayed=>"Delayed: execute once after the configured delay.",MacroType::CountedRepeat=>"Counted Repeat: execute Count times at the selected interval.",MacroType::Alternate=>"Alternate: execute Action A, then Action B on successive presses."}}
fn action_label(a:ActionType)->&'static str{match a{ActionType::Message=>"Send Message",ActionType::Command=>"Execute Command",ActionType::FillChat=>"Fill Chat Without Send",ActionType::LocalMessage=>"Display Local Message"}}
fn modifier_label(m:Modifier)->&'static str{match m{Modifier::None=>"Modifier: None",Modifier::Shift=>"Modifier: Shift",Modifier::Control=>"Modifier: Control",Modifier::Alt=>"Modifier: Alt"}}
fn binding_label(m:&Macro)->String{if m.key.is_empty(){return "Unbound (click to set)".into();}let key=m.key.strip_prefix("key.keyboard.").or_else(||m.key.strip_prefix("key.")).unwrap_or(&m.key);let prefix=match m.modifier{Modifier::None=>"",Modifier::Shift=>"Shift + ",Modifier::Control=>"Ctrl + ",Modifier::Alt=>"Alt + "};format!("{prefix}{key}")}
fn validate(d:&Draft)->Result<(),String>{
    if d.value.name.trim().is_empty(){return Err("Enter a macro name.".into());}
    if d.value.action.text.trim().is_empty(){return Err("Action A cannot be empty.".into());}
    if d.value.kind==MacroType::Alternate&&d.value.second_action.text.trim().is_empty(){return Err("Alternate requires Action B.".into());}
    if d.value.kind==MacroType::Delayed&&d.delay.parse::<u64>().is_err(){return Err("Cooldown / delay must be a whole number of milliseconds (0 or greater).".into());}
    if matches!(d.value.kind,MacroType::Toggle|MacroType::Repeat|MacroType::CountedRepeat)&&d.interval.parse::<u64>().is_err(){return Err("Interval must be a whole number of milliseconds (0 or greater).".into());}
    if d.value.kind==MacroType::CountedRepeat&&d.count.parse::<u32>().map_or(true,|n|n==0){return Err("Count must be a whole number from 1 to 4294967295.".into());}Ok(())
}
fn wheel_sector(p:[f32;2],center:[f32;2],inner:f32,outer:f32)->Option<usize>{let dx=p[0]-center[0];let dy=p[1]-center[1];let distance=dx.hypot(dy);if distance<inner||distance>outer{return None;}let angle=(dy.atan2(dx)+std::f32::consts::FRAC_PI_2+std::f32::consts::PI/6.).rem_euclid(std::f32::consts::TAU);Some((angle/(std::f32::consts::TAU/6.)).floor() as usize%6)}
fn translation_key(s:&str)->&str{match s{
    "Keybinds Menu"=>"text.config.mainscreen","Global Macros"=>"text.globalmacros.title","Server Macros"=>"text.servermacros.title","Wheel options"=>"text.wheel.options.title","Create Macro"=>"text.createmacro","Edit Macro"=>"text.editmacro","Search for macros"=>"text.searchbox.shadow","Macro name"=>"text.editmacro.field.name","Action A type"=>"text.editmacro.field.actiontype.first","Action B type"=>"text.editmacro.field.actiontype.second","Action A"=>"text.editmacro.field.action.first","Action B"=>"text.editmacro.field.action.second","Macro type"=>"text.editmacro.field.macrotype","Cooldown (ms)"|"Delay (ms)"=>"text.editmacro.field.timing","Interval (ms)"=>"text.editmacro.field.interval","Count"=>"text.editmacro.field.count","Key binding"=>"text.editmacro.field.keybind","Simple"=>"text.type.simple","Toggle"=>"text.type.toggle","Repeat"=>"text.type.repeat","Delayed"=>"text.type.delayed","Counted Repeat"=>"text.type.countedrepeat","Alternate"=>"text.type.alternate","Send Message"=>"text.action.message","Execute Command"=>"text.action.command","Fill Chat Without Send"=>"text.action.fillchat","Display Local Message"=>"text.action.localmessage","Delete this macro?"=>"text.macro.deleteQuestion","Delete this macro"=>"text.macro.deleteButton","Stop all running macros"=>"text.stopmacro","Empty"=>"text.wheel.empty","Disabled"=>"text.wheel.disabled","Remove macro"=>"text.wheel.remove","Change icon"=>"text.wheel.change.icon","Remove icon"=>"text.wheel.remove.icon","Select a macro"=>"text.wheel.select.macro","Select an icon"=>"text.wheel.select.icon","No macro available"=>"text.wheel.no.macros",_=>s}}
