#!/bin/bash
set -e

# Skrip rilis otomatis untuk Claw10 OS ke crates.io
# Jalankan skrip ini di terminal lokal Anda setelah melakukan login.

if [ -z "$1" ]; then
    echo "Penggunaan: ./publish.sh <API_TOKEN_CRATES_IO>"
    exit 1
fi

TOKEN=$1

# Ambil versi lokal saat ini dari file VERSION
VERSION=$(cat VERSION 2>/dev/null || echo "0.2.4")
echo "Versi target rilis: v$VERSION"

echo "Melakukan login ke crates.io..."
cargo login "$TOKEN"

# Daftar crate diurutkan berdasarkan pohon dependensi (daun dependensi paling bawah dirilis terlebih dahulu)
CRATES=(
    "claw10-domain"
    "claw10-toon"
    "claw10-model-router"
    "claw10-telemetry"
    "claw10-store"
    "claw10-memory"
    "claw10-scheduler"
    "claw10-skill"
    "claw10-budget"
    "claw10-auth"
    "claw10-event"
    "claw10-context"
    "claw10-policy"
    "claw10-icvs"
    "claw10-prompt"
    "claw10-lifecycle"
    "claw10-lineage"
    "claw10-mission"
    "claw10-task"
    "claw10-worker"
    "claw10-tool"
    "claw10-spawn"
    "claw10-control-api"
    "claw10-tui"
    "claw10-gateway"
    "claw10-artifact"
    "claw10-agent"
    "claw10" # Binary utama
)

cleanup_cron() {
    if grep -q "schedule:" .github/workflows/publish.yml; then
        echo "Seluruh crate v$VERSION telah terbit. Menghapus scheduler cron dari workflow..."
        sed -i '/schedule:/d' .github/workflows/publish.yml
        sed -i '/- cron:/d' .github/workflows/publish.yml
        
        git config --global user.name "github-actions[bot]"
        git config --global user.email "github-actions[bot]@users.noreply.github.com"
        git add .github/workflows/publish.yml
        if git commit -m "ci: hapus scheduler cron karena seluruh crate telah dipublikasikan"; then
            BRANCH=$(git branch --show-current 2>/dev/null || echo "master")
            git push origin "$BRANCH" || echo "Warning: Gagal push (mungkin branch diproteksi atau kredensial CI terbatas)"
        fi
    fi
}

# Cek berapa banyak crate yang belum terbit
PENDING_CRATES=0
for crate in "${CRATES[@]}"; do
    PREFIX1="${crate:0:2}"
    PREFIX2="${crate:2:2}"
    if ! curl -s -f "https://index.crates.io/${PREFIX1}/${PREFIX2}/${crate}" | grep -q "\"vers\":\"${VERSION}\""; then
        PENDING_CRATES=$((PENDING_CRATES + 1))
    fi
done

if [ "$PENDING_CRATES" -eq 0 ]; then
    echo "Semua crate v$VERSION sudah terbit di crates.io."
    cleanup_cron
    exit 0
fi

echo "Memulai publikasi ke crates.io..."
for crate in "${CRATES[@]}"; do
    echo "========================================"
    echo "Mempublikasikan: $crate"
    echo "========================================"

    # Cek apakah versi ini sudah terbit di crates.io via CDN sparse index (mengurangi rate limit)
    PREFIX1="${crate:0:2}"
    PREFIX2="${crate:2:2}"
    if curl -s -f "https://index.crates.io/${PREFIX1}/${PREFIX2}/${crate}" | grep -q "\"vers\":\"${VERSION}\""; then
        echo ">> Crate $crate v$VERSION sudah terbit di crates.io (dideteksi via sparse index). Lewati."
        continue
    fi
    
    # Menjalankan publish dengan penanganan error crate sudah ada (already exists)
    if ! out=$(cargo publish -p "$crate" --allow-dirty 2>&1); then
        echo "$out"
        if echo "$out" | grep -q -E "already exists|already published"; then
            echo ">> Crate $crate sudah terbit di crates.io. Lewati."
        else
            echo ">> Gagal mempublikasikan $crate. Menghentikan skrip."
            exit 1
        fi
    else
        echo "$out"
    fi
    
    echo "Menunggu 10 detik agar indeks crates.io diperbarui..."
    sleep 10
done

echo "Seluruh crate berhasil dipublikasikan!"
cleanup_cron
