I want you to create an LLM training/inference infrastructure. This is a CLI application and the application looks like this.
`my-gpt` is the name of the application.

1. `my-gpt init config.json`
  - It creates a `config.json` file.
  - It looks like `{ "tokenizer": { "type": "byte" }, "model": { "d_model": 512, "n_layers": 8, "n_heads": 8, "d_ff": 2048, "max_seq_len": 512, "dropout": 0.1 }, "training": { "batch_size": 4, "learning_rate": 0.0003, "weight_decay": 0.1 } }`.
  - There are 2 types of tokenizers:
    - byte: each byte is a token. So the dictionary has 256 tokens! It'd be very easy to implement, right?
    - bpe: byte-pair-encoding tokenizer. Implement this.
2. `my-gpt pretrain config.json`
  - It reads pretraining data from `pt-data/` and trains an AI model.
  - If there's no saved checkpoints in `checkpoints/`, it trains a new model from scratch with the configs in `config.json`. If there're checkpoints, it resumes training from the last checkpoint.
  - It periodically saves checkpoints to `checkpoints/`. So, you can safely kill the process at anytime.
3. `my-gpt sft`
  - It reads supervised-fine-tuning data from `sft-data/` and trains an AI model.
  - It assumes that there're checkpoints in `checkpoints/`.
  - It loads model data from the checkpoint, and continues sft.
  - It also periodically saves checkpoints to `checkpoints/`.
4. `my-gpt infer <prompt>`
  - The model generates texts with the given prompt.
  - It uses the last checkpoint in `checkpoints/`.

Some extra notes for the data directories:

1. `pt-data/`
  - You'll find a lot of txt files in `pt-data/`.
  - Each text file is 500KiB ~ 1MiB long.
2. `sft-data/`
  - You'll find a lot of json files in `sft-data/`.
  - Each json file looks like this: `[{ "q": "What is 1 + 1?", "a": "It's 2" }, { "q": "What is the capital of France?", "a": "Paris" }, ... and a lot more qa pairs]`

I want the application to be written in Rust. You can use whatever library that implements transformer training/inference well.
I want the application to run in CPU-only mode. I cannot afford fancy GPUs, but I have a lot of machines with fancy CPUs and large RAMs.
Currently, my machine's specs are:

1. Intel Ultra 7 155H CPU, 32G RAM, no GPU
  - This CPU has 12 cores.
2. Apple M3 Pro CPU, 18G RAM, Apple M3 Pro GPU
  - This CPU has 10 cores.
  - I'm not sure whether it has a GPU...
3. Intel i7-12700k CPU, 32G RAM, nvidia 3060 GPU
  - This CPU has at least 12 cores. (I don't remember the exact number.)
4. Amazon EC2: 32 cores CPU, 64G RAM

Make sure that the program runs on all of my 4 machines.
