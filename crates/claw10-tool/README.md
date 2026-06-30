# claw10-tool

Registri dan implementasi perkakas (*tool registry*) bawaan untuk **Claw10 OS**.

Crate ini mendefinisikan trait `Tool` asinkron dan mengelola registri seluruh perkakas yang dapat dipanggil oleh agen. Ia juga berisi perkakas bawaan sistem seperti manipulasi berkas (*filesystem*), pemanggilan API web (*HTTP client*), dan eksekusi perintah terminal (*shell*).

## Fitur Utama
* **Built-in Tools**:
  * **Filesystem**: `read_file`, `write_file`, `list_dir`.
  * **HTTP**: `http_request`.
  * **Shell**: `execute_command`.
* **Tool Registry**: Registrasi perkakas kustom secara dinamis beserta definisi argumen masukan dalam skema JSON.

## Cara Penggunaan
```toml
[dependencies]
claw10-tool = { workspace = true }
```
