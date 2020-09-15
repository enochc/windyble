docker run \
--volume /Users/enochcarter/windyble:/home/cross/project \
--volume /Users/enochcarter/.cargo/registry:/home/cross/.cargo/registry \
 vonamos/rust_berry:latest build --release
