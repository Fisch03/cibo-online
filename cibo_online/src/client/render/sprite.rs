use monos_gfx::{
  ui::{UIContext, UIFrame}, Framebuffer, Image, Position, input::Input, Rect
};

pub struct Sprite<'a> {
  position: Position,
  image: &'a Image,
  ui: Option<(UIFrame, Box<dyn FnMut(&mut UIContext)>)>,
  rect: Rect,
}

impl <'a, F> Sprite<'a, F> where F: Fn(&mut UIContext) {
  pub fn new(position: Position, image: &'a Image) -> Self {

    Sprite {
      position,
      image,
      ui: None,
      rect: Rect::new(Position::new(0, i64::MIN), Position::from_dimensions(image.dimensions())),
    }
  }

  pub fn with_ui(mut self, ui: UIFrame, f: F) -> Self {
    self.ui = Some((ui, f));
    self
  }

  pub fn render(&self, framebuffer: &mut Framebuffer, input: &mut Input) {
    framebuffer.draw_img(self.image, &self.position);
    if let Some((ui_frame, f)) = &self.ui {
      let rect = self.rect.translate(self.position);
      ui_frame.draw_frame(framebuffer, rect, input, f);
    }
  }
}