#!/bin/bash

mkdir -p src-tauri/resources/models/ner
cd src-tauri/resources/models/ner

# Download DistilBERT-NER model files from Hugging Face
BASE_URL="https://huggingface.co/elastic/distilbert-base-cased-finetuned-conll03-english/resolve/main"

curl -L -o model.safetensors "$BASE_URL/model.safetensors"
curl -L -o vocab.txt "$BASE_URL/vocab.txt"
curl -L -o tokenizer.json "https://huggingface.co/distilbert-base-cased/resolve/main/tokenizer.json"
curl -L -o config.json "$BASE_URL/config.json"

echo "Done! Files downloaded:"
ls -lh
