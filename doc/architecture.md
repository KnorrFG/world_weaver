# Architecture

World Weaver is split into two crates: gui and engine. Gui contains the UI, the
engine the UI independent logic. The key-type in this crate is `game::Game`.
A game consists of an LLM, and image model and its data.

The main-method of `Game` is `send_to_llm` it takes turn inputs and produces 3 things:
- A future that will return the Generated image,
- a text stream that will output the LLM response as it comes in,
- and a future with the complete turn output, once everything is complete

This will not change the game in any way. Instead, once all the futures returned,
`Game::update` must be called. This may seem inconvenient, but this design makes it
so that the game-object will not be borrowed by `send_to_llm`s outputs, which makes life SO
much easier. iced comes with an async-runtime, and I use the "tokio" feature, because
reqwest requires tokio. Nothing is thread/async-safe. The code is modeled
in a way that makes that unnecessary, which I like much more than worrying about
synchronization everywhere.

## AI Models

An LLM is anything that implements `llm::LLM`. Currently, there is only `llm::claude::Claude`
but adding more would be trivial, just take the existing model as example.
Similarly, an image model is anything that implements `image_model::ImageModel`
Currently there are two types that do that: `image_model::flux2::Flux2` and `image_model::replicate::Replicate`.
You can use the `image_model::ProvidedModel` type to instantiate models. Adding another
model that is hosted on Replicate will be very easy, but you can also add new Providers.
Make sure to adjust the `Model`, `Provider` and `ProvidedModel` enums. `Model` is Flux1 or Flux2
`Provider` is Replicate or Black Forest Labs. A `ProvidedModel` is a combination of the two.
However, not all combinations of providers and models are allowed, therefore it's an enum.

## Game Data

One of the most important data structures in this program is `game::GameData` it's defined
in *engine/src/game.rs* and looks like this:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameData {
    pub world_description: WorldDescription,
    pub pc: String,
    pub summaries: Vec<Summary>,
    pub turn_data: Vec<TurnData>,
}

```
(unless I change it, and forget to update this document)

The `world_description` is everything you type into the World-Editor,
`pc` is the player characters name. It is assumed that the `world_description`
contains a character of that name.

Every 8 turns, the world-description, the last summary (if it exists) and the
inputs and outputs of the last 8 turns will be sent to the LLM to update/generate
a new summary. Those summaries are stored in the `summaries` field.

A `TurnData` contains all relevant inputs and ouputs of a single turn.
Images in the turn data are referenced by IDs (see next section).

## Data Storage

Generating an Image every turn leads to quite a bit of data, I want single-file
saves, and I don't want to rewrite the whole file when I add an image. I didn't
find an archive format that would give me that, so I wrote a
custom save-archive format. It's in `game::save_archive`. As long
as a game is running, the gui will hold an archive open. When an image
is received, it's added to the archive, the IDs in the `TurnData` objects
are managed by the archive.

All details about the Archive format are in the module doc-string.

## The Gui

To understand the GUI, you need to understand
[the Elm architecture](https://book.iced.rs/architecture.html). 
On top of it, the GUI defines a state machine, this is the state trate:

```rust
pub trait State: fmt::Debug {
    fn update(&mut self, event: UiMessage, ctx: &mut Context) -> Result<StateCommand>;
    fn view<'a>(&'a self, ctx: &'a Context) -> Element<'a, UiMessage>;
    fn clone(&self) -> Box<dyn State>;
}
```

Each visual mode/menu is one state. Each state has it's own message type. All
messages are defined in *gui/src/message.rs*.

The `StateCommand` that is returned by `update` looks like this:
```rust
#[derive(Debug, Default)]
pub struct StateCommand {
    pub task: Option<Task<Message>>,
    pub transition: Option<Box<dyn State>>,
}
```
It's an extension of iced's tasks by the ability to change the ui state.
States don't hold any important data, and can safely be discarded at any time.
The important data lives in the `Context` that is passed to to each state.
It looks like this:

```rust
pub struct Context {
    pub game: Option<game_context::GameContext>,
    pub config: Config,
}
```

The `GameContext` is special, because it can also receive messages, as you can
see from the guis `try_update` method:

```rust
fn try_update(&mut self, message: Message) -> Result<Task<Message>> {
      match message {
          Message::Ui(ui_message) => {
              let cmd = self.state.update(ui_message, &mut self.ctx)?;
              if let Some(new_state) = cmd.transition {
                  self.state = new_state;
              }
              Ok(cmd
                  .task
                  .map(|t| t.map(Message::from))
                  .unwrap_or(Task::none()))
          }
          Message::Context(context_message) => self.ctx.update(context_message),
      }
  }
```

The `GameContext` keeps track of the running game along with all additional data
required, even when the playing state is discarded.

```rust
pub struct GameContext {
    pub game: Game,
    pub save: SaveArchive,
    pub sub_state: SubState,
    pub output_text: String,
    pub current_generation: usize,

    // UI helpers, not important for internal logic
    pub output_markdown: Vec<markdown::Item>,
    pub image_data: Option<(ImgHandle, String)>,
}
```

The `GameContext` grew out of the playing state when I realized I want to be able
to go back to the main-menu while a response is being streamed from the LLM without
losing data. As such, it has the substates that are relevant while playing:

```rust
#[derive(Debug, Default, Clone, From, TryInto)]
pub enum SubState {
    #[default]
    Uninit,
    Complete(Complete),
    WaitingForOutput(WaitingForOutput),
    WaitingForSummary(WaitingForSummary),
    InThePast(InThePast),
}
```

`Complete` means no future or stream that was run by `Game::send_to_llm` is active,
`InThePast` means the player pressed the back button, and the displayed output is
not the latest available one. All other substates should be clear. Most methods
in the `GameContext` are called somewhere in the `Playing` state, so it makes more
sense to look at that.

`output_markdown` needs to be parsed from the `output_text`. Since I don't want
to do this in `Playing::view` everytime, I do it here. `image_data` contains the
image in a form that iced can render directly (the `ImgHandle`) and the image's
caption. Parsing the image bytes into an `ImgHandle` is also something that I don't
want to do on every `Playing::view` call, so it's done here. The `output_text`
holds the turns output as it's coming in from the stream.

The `current_generation` field is there for error handling. If the LLM violates
the output format or Flux2, in all its wisdom (/s),
decides that your image should not be rendered, there
will be an error and the turn can't be completed. However, other events from
the invalid turn might still come in. So we mark those with a generation.
When an error occurs, we reset the substate, increase the generation, and ignore
all events from older generations. 

