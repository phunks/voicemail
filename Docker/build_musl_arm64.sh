cross build --target aarch64-unknown-linux-musl --release
cd ..
docker buildx build -f Docker/Dockerfile_musl_arm64 --platform linux/arm64 -t voicemail/arm64 .
docker save voicemail/arm64:latest > Docker/voicemail_arm64.tar
