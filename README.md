# Cibo Online
is a social hangout "game" where you can talk and interact with other players, inspired by the vtuber [Mono Monet](https://www.youtube.com/@MonoMonet) of V4Mirai (think: [Yume Nikki Online](ynoproject.net) but for tabemonos :3).
Cibo Online is a part of [monOS](https://github.com/Fisch03/monOS/), but can also be played in the browser without any installation

## project structure
- [`cibo_online`](https://github.com/Fisch03/cibo-online/tree/master/cibo_online) provides most of the client functionality as well as shared definitions for client and server
- [`server`](https://github.com/Fisch03/cibo-online/tree/master/server) contains the game server code
- [`web_client`](https://github.com/Fisch03/cibo-online/tree/master/server) contains the webassembly client
- the source for the client shipped with monOS is available in the monOS source tree [here](https://github.com/Fisch03/monOS/tree/master/userspace/cibo_online)

## hosting your own server
should be a simple `cargo run` in the workspace root :) you will need to install [wasm-pack](https://rustwasm.github.io/wasm-pack/) first
