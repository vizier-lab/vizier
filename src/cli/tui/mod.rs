use std::{sync::Arc, time::Duration};

use cursive::{
    event::Event,
    theme::{BaseColor, Color, ColorStyle, PaletteColor},
    utils::markup::StyledString,
    view::{IntoBoxedView, Margins, Nameable, Resizable},
    views::{
        Button, Dialog, DummyView, LinearLayout, PaddedView, ScrollView, TextView, ThemedView,
    },
};

mod placeholder_edit_view;
use futures::{SinkExt, StreamExt};
use placeholder_edit_view::PlaceholderEditView;

use anyhow::Result;
use clap::Args;
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::{
    channels::http::{
        models::response::APIResponse, models::session::ChatRequest, models::session::ChatResponse,
        models::session::SessionResponse,
    },
    config::VizierConfig,
};

#[derive(Debug, Args, Clone)]
pub struct TuiArgs {
    #[arg(short, long, help = "path to .vizier.toml config file")]
    pub base_url: Option<String>,

    #[arg(
        short,
        long,
        value_name = "PATH",
        value_hint = clap::ValueHint::DirPath,
        help = "path to .vizier.toml config file",
    )]
    pub config: Option<std::path::PathBuf>,
}

fn status_label(label: &str, value: &str) -> StyledString {
    let mut styled = StyledString::plain(label);
    styled.append(StyledString::styled(
        value,
        ColorStyle::new(Color::Light(BaseColor::Blue), Color::TerminalDefault),
    ));
    styled
}

fn chat_message(actor: &str, value: String, is_agent: bool) -> impl IntoBoxedView {
    let speaker = if !is_agent {
        StyledString::styled(
            actor,
            ColorStyle::new(Color::Light(BaseColor::Blue), Color::TerminalDefault),
        )
    } else {
        StyledString::styled(
            actor,
            ColorStyle::new(Color::Light(BaseColor::Green), Color::TerminalDefault),
        )
    };

    let content = LinearLayout::horizontal()
        .child(TextView::new(value).full_width())
        .full_width();

    let dialog = Dialog::around(content)
        .title(speaker)
        .title_position(if is_agent {
            cursive::align::HAlign::Right
        } else {
            cursive::align::HAlign::Left
        })
        .full_width();

    if is_agent {
        LinearLayout::horizontal()
            .child(DummyView::new().fixed_width(50))
            .child(dialog)
            .full_width()
    } else {
        LinearLayout::horizontal()
            .child(dialog)
            .child(DummyView::new().fixed_width(50))
            .full_width()
    }
}

pub async fn run(args: TuiArgs) -> Result<()> {
    let base_url = if let Some(path) = args.config {
        let config = VizierConfig::load(Some(path))?;

        format!("localhost:{}", config.channels.http.unwrap().port)
    } else {
        args.base_url.unwrap()
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    // create session
    let response = client
        .post(format!(
            "http://{}/session/{}",
            base_url,
            nanoid::nanoid!(10)
        ))
        .send()
        .await?;

    let session_value: APIResponse<SessionResponse> =
        serde_json::from_str(&response.text().await?)?;
    let session_id = session_value.data.unwrap().session_id;

    let (input_writer, input_reader) = flume::unbounded::<ChatRequest>();
    let (output_writer, output_reader) = flume::unbounded::<ChatResponse>();
    let output_writer = Arc::new(output_writer);

    // let loading_state = vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    let mut siv = cursive::default();

    // Use terminal's own colors (respects dark/light terminal theme)
    let mut theme = siv.current_theme().clone();
    theme.palette[PaletteColor::Background] = Color::TerminalDefault;
    theme.palette[PaletteColor::View] = Color::TerminalDefault;
    theme.palette[PaletteColor::Primary] = Color::TerminalDefault;
    theme.palette[PaletteColor::Secondary] = Color::TerminalDefault;
    theme.palette[PaletteColor::Tertiary] = Color::TerminalDefault;
    siv.set_theme(theme);

    let message_area = Dialog::new()
        .content(
            LinearLayout::vertical().child(
                ScrollView::new(
                    LinearLayout::vertical()
                        .child(DummyView.fixed_height(1))
                        .with_name("content"),
                )
                .scroll_strategy(cursive::view::ScrollStrategy::StickToBottom)
                .full_height(),
            ),
        )
        .title("vizier")
        .full_height()
        .full_width();

    let input_area = Dialog::new().content(
        LinearLayout::horizontal()
            .child(PaddedView::new(
                Margins::lr(0, 2),
                PlaceholderEditView::new("Type a message...")
                    .on_submit(move |s: &mut cursive::Cursive, msg| {
                        let _ = input_writer.send(ChatRequest {
                            user: "user".into(),
                            content: msg.to_string(),
                        });

                        s.call_on_name("content", |content: &mut LinearLayout| {
                            let message = chat_message("you", msg.to_string(), false);
                            content.add_child(message);
                        });
                        s.call_on_name("input", |input: &mut PlaceholderEditView| {
                            input.clear();
                        });
                    })
                    .with_name("input")
                    .full_width(),
            ))
            // .child(Button::new_raw("[Send]", |s| {}))
            .full_width(),
    );

    let status_area = Dialog::new()
        .content(
            LinearLayout::vertical()
                .child(
                    LinearLayout::vertical()
                        .child(TextView::new(status_label("host: ", &base_url)))
                        .child(TextView::new(status_label(
                            "sesion_id: ",
                            &session_id.to_string(),
                        )))
                        .child(TextView::new("status: "))
                        .full_height(),
                )
                .child({
                    let mut quit_theme = siv.current_theme().clone();
                    quit_theme.palette[PaletteColor::Primary] = Color::Light(BaseColor::Red);
                    ThemedView::new(
                        quit_theme,
                        Button::new_raw("[Quit]", |s| {
                            s.quit();
                        }),
                    )
                }),
        )
        .title("status")
        .full_height()
        .fixed_width(30);

    siv.add_fullscreen_layer(
        LinearLayout::horizontal()
            .child(
                LinearLayout::vertical()
                    .child(message_area)
                    .child(input_area),
            )
            .child(status_area),
    );

    let ws_url = format!("ws://{}/session/{}/chat", base_url, session_id);
    let (ws, _) = connect_async(ws_url).await.unwrap();
    let (mut ws_write, mut ws_read) = ws.split();

    let ws_res_handle = tokio::spawn(async move {
        while let Some(Ok(message)) = ws_read.next().await {
            if message.is_text() {
                let res = serde_json::from_str(&message.to_string()).unwrap();
                let _ = output_writer.send_async(res).await;
            }
        }
    });

    let ws_req_handle = tokio::spawn(async move {
        loop {
            if let Ok(content) = input_reader.try_recv() {
                let req = serde_json::to_string(&content).unwrap();

                let _ = ws_write.send(Message::Text(req.into())).await;
            }
        }
    });

    siv.add_global_callback(Event::Refresh, move |s| {
        s.call_on_name("content", |content: &mut LinearLayout| {
            while let Ok(response) = output_reader.try_recv() {
                if response.thinking {
                    continue;
                }

                let chat = chat_message("vizier", response.content, true);
                content.add_child(chat);
            }
        });
    });

    siv.set_autorefresh(true);
    siv.run();
    ws_req_handle.abort();
    ws_res_handle.abort();
    Ok(())
}
