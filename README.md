# CRIU batch converter

A simple script that will convert videos highly compressed videos with [AV1](https://en.wikipedia.org/wiki/AV1) as the video codec and [Opus](https://en.wikipedia.org/wiki/Opus_(audio_format)) as the audio codec.

Since video conversion using the AV1 codeec is horrendously slow, a checkpointing program, `criu`, will be used to persist conversion states across reboots, program shutdowns, etc.

# Usage

To add a single file into conversion queue, do:

```bash
./criu-converter add DATABASE_PATH FILE
```

To add multiple files into conversion queue, do:

```bash
./criu-converter add DATABASE_PATH FILE [FILE]
```

To add multiple files and stitch them together in one video, do:

```bash
./criu-converter add -m DATABASE_PATH FILE [FILE]
```

To initialize conversion, do:

```bash
./criu-converter convert DATABASE_PATH CHECKPOINTS_PATH
```

Initialization of conversion usually requires privilege escalation (`sudo`).
