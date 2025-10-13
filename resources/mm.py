import os
import subprocess

# --- Config ---
input_folder = "./sounds"
output_folder = "./ogg"

# Create output folder if it doesn't exist
os.makedirs(output_folder, exist_ok=True)

# Iterate over all WAV files in input folder
for filename in os.listdir(input_folder):
    if filename.lower().endswith(".wav"):
        wav_path = os.path.join(input_folder, filename)
        ogg_filename = os.path.splitext(filename)[0] + ".ogg"
        ogg_path = os.path.join(output_folder, ogg_filename)
        
        # Run ffmpeg to convert WAV -> OGG (Vorbis)
        subprocess.run([
            "ffmpeg",
            "-y",                # overwrite output if exists
            "-i", wav_path,
            "-c:a", "libvorbis",
            ogg_path
        ], check=True)
        
        print(f"Converted {filename} -> {ogg_filename}")

print("All files converted successfully!")
