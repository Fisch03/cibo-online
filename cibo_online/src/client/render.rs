use super::{ClientGameState, ClientMessage};

use monos_gfx::{types::*, Framebuffer};

impl ClientGameState {
    pub(super) fn render(
        &mut self,
        framebuffer: &mut Framebuffer,
        send_msg: &mut dyn FnMut(ClientMessage),
    ) {
        let fb_rect = Rect::from_dimensions(framebuffer.dimensions());

        self.local
            .ui_frame
            .draw_frame(framebuffer, fb_rect, &mut self.local.input, |ui| {
                ui.label("good mononing!");

                if let Some(chat) = &mut self.local.own_chat {
                    if ui.textbox(chat).submitted {
                        send_msg(ClientMessage::Chat(chat.clone()));
                        self.local.own_chat = None;
                    }
                }
            });

        framebuffer.draw_rect(
            &Rect::new(
                Position::new(self.client.position().x, self.client.position().y),
                Position::new(self.client.position().x + 32, self.client.position().y + 32),
            ),
            &Color::new(255, 255, 255),
        );

        for client in &self.game_state.clients {
            let position = client.position();
            framebuffer.draw_rect(
                &Rect::new(
                    Position::new(position.x, position.y),
                    Position::new(position.x + 32, position.y + 32),
                ),
                &Color::new(255, 255, 255),
            );
        }
    }
}
