# The Studio Voice model

TidyCuts' **Studio Voice** clean-up runs the *MossFormer2_SE_48K* speech-enhancement
network from [ClearerVoice-Studio](https://github.com/modelscope/ClearerVoice-Studio)
(Apache License 2.0; the weights' model card says Apache-2.0 too) through ONNX Runtime.

The network is 217 MB, so it is not kept in git. It is attached to the GitHub release
named `voice-model-1` as `mossformer2_se_48k.onnx`, and `scripts/fetch-voice-assets.*`
download it, check its SHA-256 (see `packaging/voice-assets.txt`) and put it next to the
program when the installers are built.

## Making the file yourself

```
python -m venv env && env/Scripts/pip install clearvoice onnx onnxruntime onnxscript
env/Scripts/python tools/voice-model/export_mossformer2.py
```

This downloads the original weights from Hugging Face, exports them with a fixed
window of 496 feature frames (4 seconds), and prints how far the export differs from
the original (about 0.00002). The exported file should hash to the value in
`packaging/voice-assets.txt` when made with the same package versions; if you export
again with other versions, update the hash and upload the new file to a new release
tag (`voice-model-2`, and so on).

## What TidyCuts does around it

`crates/tidycuts-engine/src/studio_voice.rs` prepares what the network was trained on
(48 kHz sound, 40 ms Hamming windows every 8 ms, a 60-band mel filter bank with its two
deltas), runs it on overlapping 4-second windows, applies the mask it returns and builds
the sound back. A test compares those steps with the original Python code on real
sound (`TIDYCUTS_VECTORS`, see the test's comment).

## Without the files

The program still works: Voice Clean-up then uses the small built-in RNNoise engine
(`voice_clean.rs`), which removes less noise but needs no extra files.
