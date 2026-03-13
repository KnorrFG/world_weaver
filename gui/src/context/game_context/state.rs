use std::mem;

use color_eyre::{
    Result,
    eyre::{eyre, ErrReport},
};
use derive_more::{From, TryInto};
use engine::game::TurnData;

use crate::context::game_context::pending_turn::{FinalizingTurn, PendingTurn};

#[derive(Debug, Default, Clone, From, TryInto)]
pub enum SubState {
    #[default]
    Uninit,
    Complete(Complete),
    WaitingForOutput(PendingTurn),
    WaitingForSummary(FinalizingTurn),
    InThePast(InThePast),
}

#[derive(Debug, Clone)]
pub struct Complete {
    pub turn_data: TurnData,
}

#[derive(Debug, Clone)]
pub struct InThePast {
    pub completed_turn: usize,
    pub data: TurnData,
}

impl SubState {
    pub fn stream_buffer_mut(&mut self) -> Result<&mut String> {
        if let SubState::WaitingForOutput(pending_turn) = self {
            Ok(pending_turn.stream_buffer_mut())
        } else {
            Err(eyre!("Can't provide stream_buffer while being: {self:#?}"))
        }
    }

    pub fn take(&mut self) -> Self {
        mem::take(self)
    }

    pub fn turn_data(&self) -> Result<&TurnData, ErrReport> {
        match self {
            Self::Complete(Complete { turn_data }) => Ok(turn_data),
            Self::InThePast(InThePast { data, .. }) => Ok(data),
            _ => Err(eyre!("Trying to get turn-data while being: {self:?}")),
        }
    }
}
