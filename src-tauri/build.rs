use std::fs;
use std::path::Path;

const MODEL_FILES: &[(&str, &str)] = &[
    (
        "model.safetensors",
        "https://huggingface.co/elastic/distilbert-base-cased-finetuned-conll03-english/resolve/main/model.safetensors",
    ),
    (
        "config.json",
        "https://huggingface.co/elastic/distilbert-base-cased-finetuned-conll03-english/resolve/main/config.json",
    ),
    (
        "vocab.txt",
        "https://huggingface.co/elastic/distilbert-base-cased-finetuned-conll03-english/resolve/main/vocab.txt",
    ),
    // Get tokenizer.json from the base distilbert-base-cased model
    (
        "tokenizer.json",
        "https://huggingface.co/distilbert-base-cased/resolve/main/tokenizer.json",
    ),
];

fn main() {
    tauri_build::build();

    // Download model files if they don't exist
    let model_dir = Path::new("resources/models/ner");

    if !model_dir.join("model.safetensors").exists() {
        println!("cargo:warning=Downloading NER model files...");
        download_model_files(model_dir);
    }
}

fn download_model_files(model_dir: &Path) {
    fs::create_dir_all(model_dir).expect("Failed to create model directory");

    for (filename, url) in MODEL_FILES {
        let output_path = model_dir.join(filename);

        if output_path.exists() {
            println!("cargo:warning={} already exists, skipping", filename);
            continue;
        }

        println!("cargo:warning=Downloading {}...", filename);

        // Use curl command (available on macOS, Linux, Windows with Git)
        let status = std::process::Command::new("curl")
            .args([
                "-L",
                "--fail",
                "--silent",
                "--show-error",
                "-o",
                output_path.to_str().unwrap(),
                url,
            ])
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("cargo:warning=Downloaded {}", filename);
            }
            _ => {
                panic!(
                    "Failed to download {}. Run manually:\ncurl -L -o {} {}",
                    filename,
                    output_path.display(),
                    url
                );
            }
        }
    }
}
