# claw10-model-router

Router penyedia LLM (*Language Model Provider Router*) untuk **Claw10 OS**.

Crate ini menyediakan abstraksi terpadu untuk berkomunikasi dengan berbagai model kecerdasan buatan dari penyedia eksternal seperti OpenAI, Anthropic, Gemini, DeepSeek, Cohere, Fireworks, Perplexity, Together, xAI, OpenRouter, Nvidia, dan penyedia lokal (Ollama/Groq).

## Fitur Utama
* **Abstraksi Multi-Provider**: Antarmuka tunggal untuk pengiriman pesan, pemanggilan perkakas (*tool calling*), dan streaming respon dari berbagai LLM.
* **OpenAI-Compatibility**: Mendukung model alternatif yang kompatibel dengan protokol API OpenAI.

## Cara Penggunaan
```toml
[dependencies]
claw10-model-router = { workspace = true }
```
