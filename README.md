<p align="center">
<img height="125" src="https://github.com/Fisch03/cibo-online/blob/master/assets/logo.png" />
</p>

is a social hangout "game" where you can talk and interact with other players, inspired by the vtuber [Mono Monet](https://www.youtube.com/@MonoMonet) of V4Mirai (think: [Yume Nikki Online](ynoproject.net) or [Become Fumo](https://www.roblox.com/games/6238705697/Become-Fumo) but for tabemonos :3).
Cibo Online! is a part of [monOS](https://github.com/Fisch03/monOS), the (heavily WIP) mono-themed operating system but can also be played in the browser without any installation

## project structure
- [`cibo_online`](https://github.com/Fisch03/cibo-online/tree/master/cibo_online) provides most of the client functionality as well as shared definitions for client and server
- [`server`](https://github.com/Fisch03/cibo-online/tree/master/server) contains the game server code
- [`web_client`](https://github.com/Fisch03/cibo-online/tree/master/server) contains the wasm client source

## a note about performance
its horrible. i know. for maximum compatibility with the monOS version, i currently draw onto the raw framebuffer with zero gpu acceleration whatsoever. 
this is obviously not a great idea and will be changed/optimized at some point (probably when i have the monOS version in place).

## hosting your own server
should be a simple `cargo run` in the workspace root :) you will need to install [wasm-pack](https://rustwasm.github.io/wasm-pack/) first

## implementing your own client
i don't know _why_ you'd wanna do that, but if you want to its actually pretty simple. there are basically only three things you need to provide:
- some way of connecting to websockets
- a 640x480 linear framebuffer in the RGB(A)8 format (or any reordering - BGR8 or some other wacky variant will work just as fine)
- some sort of (keyboard) input

obtain a player name in whatever way you see fit.
initialize a `monos_gfx::Framebuffer` using your framebuffer and connect to the servers websocket. 
deserialize incoming messages into `ServerMessage`s. to join the server, send a serialized `ClientMessage::Connect` containing your requested player name. 
your client will receive a `ServerMessage::FullState` as a response containing your clients initial state that you should save.
every other type of `ServerMessage` you receive from that point on you can route straight into that saved state using its `handle_message` function.
all your client needs to do now is each frame is calling the `update` function on your `ClientGameState` to process and draw the next frame 

thats it! you can look at the wasm implementation [here](https://github.com/Fisch03/cibo-online/blob/master/web_client/src/lib.rs) to get a better idea :)
