use derive_more::{From, TryInto};
use engine::{
    game::{self, TurnOutput},
    llm,
};

use crate::StringError;

#[derive(Debug, Clone, From, TryInto)]
pub enum Message {
    Ui(UiMessage),
    Context(ContextMessage),
}

#[derive(Debug, Clone)]
pub enum ContextMessage {
    OutputComplete(Result<TurnOutput, StringError>),
    SummaryFinished(Result<Option<llm::OutputMessage>, StringError>),
    NewTextFragment(Result<String, StringError>),
    Init,
    ImageReady(Result<game::Image, StringError>),
}

#[derive(Debug, Clone, From, TryInto)]
pub enum UiMessage {
    Playing(ui_messages::Playing),
    MessageDialog(ui_messages::MessageDialog),
    ConfirmDialog(ui_messages::ConfirmDialog),
    EditDialog(ui_messages::EditDialog),
    MainMenu(ui_messages::MainMenu),
    WorldMenu(ui_messages::WorldMenu),
    WorldEditor(ui_messages::WorldEditor),
    InputDialog(ui_messages::InputDialog),
    StartNewGame(ui_messages::StartNewGame),
    LoadMenu(ui_messages::LoadMenu),
    OptionsMenu(ui_messages::OptionsMenu),
}

pub mod ui_messages {
    use super::*;

    use engine::image_model;
    use iced::widget::text_editor;

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
            UpdateActionText(text_editor::Action),
            UpdateGMInstructionText(text_editor::Action),
            ProposedActionButtonPressed(String),
            Submit,
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
            RegenerateButtonPressed,
            RegenerateMessage(String),
            ToMainMenu,
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

        pub enum InputDialog {
            Save,
            Cancel,
            Edit(String),
        }

        pub enum MainMenu {
            Continue,
            WorldsMenu,
            Options,
            SaveButton,
            Save(String),
            Load,
        }

        pub enum WorldMenu {
            NewWorld,
            StartWorld(usize)
        }

        pub enum WorldEditor {
            AddCharacterButton,
            AddCharacter(String),
            UpdateCharacter(String, text_editor::Action),
            DescriptionUpdate(text_editor::Action),
            InitActionUpdate(text_editor::Action),
            NameUpdate(String),
            Save,
            SaveAndPlay,
            Abort,
        }

        pub enum StartNewGame {
            Selected(String)
        }

        pub enum LoadMenu {
            Back,
            LoadSave(usize),
        }

        pub enum OptionsMenu {
            ClaudeTokenChanged(String),
            ImgModelTokenChanged(image_model::ModelProvider, String),
            SelectImageModel(image_model::Model),
            Ok,
        }
    }
}
