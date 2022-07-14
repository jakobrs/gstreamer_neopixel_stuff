RUSTFLAGS="-C target-cpu=cortex-a53" cross build --target armv7-unknown-linux-gnueabihf --release && rsync -avx target/armv7-unknown-linux-gnueabihf/release/avgfilter pi@domain-name.xyz:~/
