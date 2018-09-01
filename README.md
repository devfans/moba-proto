# moba-proto
Simple proto implementation for MOBA.

[![Build Status][travis-image]][travis-url]

## Proof of concept
Hooked with Pong example from amethyst game engine: https://youtu.be/0Yt7t5rptZY

## Player States
+ Connected
+ Init
+ Wait
+ Ready
+ Start
+ Stop


## Battle workflow

+ Server starts to listen for a battle with a specific player quantity, and a battle ID.

+ Clients connect to server and server marks clients as `Connected`.

+ Clients send battle init info  with battle ID and player ID in the message, and server will check and echo the message then mark clients as `Init` state.

+ Server checks clients state. A client in `Init` state will be converted to `Wait` state if the client is claiming an unused player ID or will be ignored for duplicate player ID. 

+ Server wait till all required players are in `Wait` state. 

+ Server drop all unknown clients(which are not in `Wait` State) and then broadcast battle metadata(including battle members, etc) to clients. 

+ Clients receive the battle metadata and confirm to server, then prepare for the battle.

+ Server receives battle metadata confirmations and marks clients as `Ready` state. Server waits all required clients to be `Ready`.

+ Server broadcast battle start requests to clients. When clients are able to start, send start confirmation to server. When server receives a battle start confirmation, it will mark the client as `Start` state.

+ Server waits all the start confirmations, and then start the battle.

+ Server spawns an interval timer to keep broardcasting data frames to clients and receives game action inputs from clients which will be used to compose data frames.

+ Clients send game actions to server while receiving data frames from server and do game computation to update battle progress on client side. All clients should make deterministic game computations to get the same battle state.

+ Battle ends, then clients send battle stop message to server and will be marked as `Stop` state by server. Server will stop the battle when received stop requests from a certain number of players.

+ Battle stops, all clients are in `Stop` state. Server and clients clean up battle state.


[travis-image]: https://img.shields.io/travis/devfans/moba-proto/master.svg
[travis-url]: https://travis-ci.org/devfans/moba-proto


