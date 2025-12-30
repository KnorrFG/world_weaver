use derive_more::{From, TryInto};

#[derive(Debug, Clone, From, TryInto)]
pub enum Message {
    Playing(state_messages::Playing),
    MessageDialog(state_messages::MessageDialog),
    ConfirmDialog(state_messages::ConfirmDialog),
    EditDialog(state_messages::EditDialog),
}

pub mod state_messages {
    use engine::{
        game::{self, TurnOutput},
        llm,
    };
    use iced::widget::text_editor;

    use crate::StringError;

    #[derive(Debug, Clone)]
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

    #[derive(Debug, Clone)]
    pub enum MessageDialog {
        Confirm,
        EditAction(text_editor::Action),
    }

    #[derive(Debug, Clone)]
    pub enum ConfirmDialog {
        Yes,
        No,
    }

    #[derive(Debug, Clone)]
    pub enum EditDialog {
        Save,
        Cancel,
        Update(text_editor::Action),
    }
}
