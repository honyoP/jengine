use crate::engine::{jEngine, Game};

pub enum SceneAction {
    None,
    Push(Box<dyn Scene>),
    Pop,
    Switch(Box<dyn Scene>),
    ReplaceAll(Box<dyn Scene>),
    Quit,
}

pub trait Scene {
    fn on_enter(&mut self, _engine: &mut jEngine) {}
    fn on_exit(&mut self, _engine: &mut jEngine) {}
    fn update(&mut self, engine: &mut jEngine) -> SceneAction;
    fn draw(&mut self, engine: &mut jEngine);
    fn is_transparent(&self) -> bool { false }
}

pub struct SceneStack {
    scenes: Vec<Box<dyn Scene>>,
    initialized: bool,
}

impl SceneStack {
    pub fn new(initial: Box<dyn Scene>) -> Self {
        Self { scenes: vec![initial], initialized: false }
    }

    fn update_inner(&mut self, engine: &mut jEngine) {
        let action = if let Some(top) = self.scenes.last_mut() {
            top.update(engine)
        } else {
            return;
        };

        match action {
            SceneAction::None => {}
            SceneAction::Push(mut s) => {
                s.on_enter(engine);
                self.scenes.push(s);
            }
            SceneAction::Pop => {
                if let Some(mut top) = self.scenes.pop() {
                    top.on_exit(engine);
                }
            }
            SceneAction::Switch(mut s) => {
                if let Some(mut top) = self.scenes.pop() {
                    top.on_exit(engine);
                }
                s.on_enter(engine);
                self.scenes.push(s);
            }
            SceneAction::ReplaceAll(mut s) => {
                while let Some(mut top) = self.scenes.pop() {
                    top.on_exit(engine);
                }
                s.on_enter(engine);
                self.scenes.push(s);
            }
            SceneAction::Quit => {
                engine.request_quit();
            }
        }
    }

    fn draw_inner(&mut self, engine: &mut jEngine) {
        let start = self.scenes.iter().rposition(|s| !s.is_transparent()).unwrap_or(0);
        for scene in &mut self.scenes[start..] {
            scene.draw(engine);
        }
    }
}

impl Game for SceneStack {
    fn update(&mut self, engine: &mut jEngine) {
        if !self.initialized {
            self.initialized = true;
            if let Some(s) = self.scenes.first_mut() {
                s.on_enter(engine);
            }
        }
        self.update_inner(engine);
    }

    fn render(&mut self, engine: &mut jEngine) {
        engine.clear();
        self.draw_inner(engine);
    }
}
