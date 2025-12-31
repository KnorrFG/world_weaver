use derive_more::{From, TryInto};

#[derive(Debug, Clone, From, TryInto)]
pub enum Message {
    Ui(UiMessage),
    Context(ContextMessage),
}

#[derive(Debug, Clone)]
pub enum ContextMessage {
    Foo,
}

#[derive(Debug, Clone, From, TryInto)]
pub enum UiMessage {
    Playing(ui_messages::Playing),
    MessageDialog(ui_messages::MessageDialog),
    ConfirmDialog(ui_messages::ConfirmDialog),
    EditDialog(ui_messages::EditDialog),
    MainMenu(ui_messages::MainMenu),
}

pub mod ui_messages {
    use super::*;
    use engine::{
        game::{self, TurnOutput},
        llm,
    };
    use iced::widget::text_editor;

    use crate::StringError;

    macro_rules! ui_enums {
        ($($pub:vis enum $name:ident { $( $variant:ident $( ( $($body:tt)* ) )? ),+ $(,)? })+) => {
            $(
                #[derive(Debug, Clone)]
                $pub enum $name {
                    $($variant $(($($body)*))?),+
                }

                impl From<$name> for Message {
                    fn from(x: $name) -> Message {
                        Message::Ui(UiMessage::$name(x))
                    }
                }
            )+
        };
    }

    ui_enums! {
        pub enum Playing {
            OutputComplete(Result<TurnOutput, StringError>),
            NewTextFragment(Result<String, StringError>),
            ImageReady(Result<game::Image, StringError>),
            Init,
            UpdateActionText(text_editor::Action),
            UpdateGMInstructionText(text_editor::Action),
            ProposedActionButtonPressed(String),
            Submit,
            SummaryFinished(Result<Option<llm::OutputMessage>, StringError>),
            PrevTurnButtonPressed,
            NextTurnButtonPressed,
            UpdateTurnInput(String),
            GotoTurnPressed,
            GoToCurrentTurn,
            LoadGameFromCurrentPastButtonPressed,
            ConfirmLoadGameFromCurrentPast,
            ShowHiddenText,
            UpdateHiddenInfo(String),
            ShowImageDescription,
            CopyInputToClipboard,
        }

        pub enum MessageDialog {
            Confirm,
            EditAction(text_editor::Action),
        }

        pub enum ConfirmDialog {
            Yes,
            No,
        }

        pub enum EditDialog {
            Save,
            Cancel,
            Update(text_editor::Action),
        }

        pub enum MainMenu {
            NewGame,
            Continue,
            WorldsMenu,
            Options,
            Save,
            Load,
        }
    }
}
