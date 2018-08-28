# moba-proto
Simple proto implementation for MOBA.

[![Build Status][travis-image]][travis-url]


## Battle workflow

+ Server starts to listen for a battle with a specific player quantity

+ Clients connect to server and send 'Battle Ready' with battle ID, player Id in the message.

+ Server checks if all required players are in 'Battle Ready' state, if not yet, will drop unknown clients and wait for rest players to be ready

+ Server broadcast battle player IDs to all and get confirmation from clients. Then broadcast 'Battle Start' message to clients.

+ Clients start the battle with sending game action inputs to server and receive data frames from server, in the mean time, server collects all the inputs and compose data frames, with a interval timer to keep broadcasting data frames to client.

+ Clients receive the data frames and do game computation to update newer battle state.

+ Client send 'Battle End' message to server. Server stop the battle and clean up.


[travis-image]: https://img.shields.io/travis/devfans/mobo-proto/master.svg
[travis-url]: https://travis-ci.org/devfans/mobo-proto


