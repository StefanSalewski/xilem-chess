# xilem-chess
First Xilem GUI for the tiny salewski chess engine

![Chess UI](http://ssalewski.de/tmp/xilem-chess.png)


### Description

This is the first Xilem GUI version for the tiny salewski chess engine.

It is based on the Xilem examples stopwatch.rs and calc.rs, and on
the threaded EGUI variant called tiny-chess.

We are currently using the latest Xilem version from GitHub.
As Xilem is still in development, it might happen that this repository
stops to compile in the future -- we will try to provide regular updates.
Currently there are no widgets for configuration available, but we should be able to
add these soon.

Automatic scaling of piece sizes when resizing the window, and dynamically changing the window title is currently not
supported due to Xilem restrictions!

### How to Run

```sh
cd /tmp
git clone https://github.com/stefansalewski/xilem-chess.git
cd xilem-chess
RUST_LOG=off cargo run
```
