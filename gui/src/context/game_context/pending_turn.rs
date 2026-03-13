use engine::game::{Image, TurnInput, TurnOutput};

#[derive(Debug, Clone)]
pub struct PendingTurn {
    pub stream_buffer: String,
    pub input: TurnInput,
    pub output: Option<TurnOutput>,
    pub image: ImageState,
}

#[derive(Debug, Clone)]
pub struct FinalizingTurn {
    pub input: TurnInput,
    pub output: TurnOutput,
    pub image: Option<Image>,
}

#[derive(Debug, Default, Clone)]
pub enum ImageState {
    #[default]
    Pending,
    Ready(Image),
    Failed,
}

pub enum Resolution {
    Pending(PendingTurn),
    Finalizing(FinalizingTurn),
}

impl PendingTurn {
    pub fn new(input: TurnInput) -> Self {
        Self {
            stream_buffer: String::new(),
            input,
            output: None,
            image: ImageState::Pending,
        }
    }

    pub fn stream_buffer_mut(&mut self) -> &mut String {
        &mut self.stream_buffer
    }

    pub fn finish_output(self, output: TurnOutput) -> Resolution {
        match self.image {
            ImageState::Ready(image) => Resolution::Finalizing(FinalizingTurn {
                input: self.input,
                output,
                image: Some(image),
            }),
            ImageState::Failed => Resolution::Finalizing(FinalizingTurn {
                input: self.input,
                output,
                image: None,
            }),
            ImageState::Pending => Resolution::Pending(Self {
                output: Some(output),
                ..self
            }),
        }
    }

    pub fn finish_image(self, image: Image) -> Resolution {
        match self.output {
            Some(output) => Resolution::Finalizing(FinalizingTurn {
                input: self.input,
                output,
                image: Some(image),
            }),
            None => Resolution::Pending(Self {
                image: ImageState::Ready(image),
                ..self
            }),
        }
    }

    pub fn fail_image(self) -> Resolution {
        match self.output {
            Some(output) => Resolution::Finalizing(FinalizingTurn {
                input: self.input,
                output,
                image: None,
            }),
            None => Resolution::Pending(Self {
                image: ImageState::Failed,
                ..self
            }),
        }
    }
}
