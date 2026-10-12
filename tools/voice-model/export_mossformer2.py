"""Exports MossFormer2_SE_48K (ClearerVoice-Studio, Apache-2.0) to ONNX with a fixed
window of 496 feature frames (4 s of 48 kHz sound), and checks the result against the
original network.

See README.md in this folder. Run it in a Python environment with:
    pip install clearvoice onnx onnxruntime onnxscript
(It downloads the original weights from Hugging Face on first run.)
"""
import torch, numpy as np, torchaudio, sys, os
from clearvoice import ClearVoice
from clearvoice.utils.misc import compute_fbank
cv=ClearVoice(task='speech_enhancement', model_names=['MossFormer2_SE_48K'])
args=cv.network_wrapper.args
m=cv.network_wrapper.network.model.eval()
T=496
class W(torch.nn.Module):
    def __init__(s,m): super().__init__(); s.m=m
    def forward(s,x): return s.m(x)[-1]
w=W(m).eval()
x=torch.randn(1,T,180)
with torch.no_grad():
    torch.onnx.export(w,(x,),"mossformer2_se_48k.onnx",input_names=["fbank"],output_names=["mask"],opset_version=17,dynamo=False)
print("exported", os.path.getsize("mossformer2_se_48k.onnx")//1024//1024,"MB")
# Check: the exported model must give the same mask as the original network.
import onnxruntime as ort
s = ort.InferenceSession("mossformer2_se_48k.onnx", providers=["CPUExecutionProvider"])
feat = torch.randn(1, T, 180)
with torch.no_grad():
    expected = m(feat)[-1].numpy()
got = s.run(None, {"fbank": feat.numpy()})[0]
print("largest difference between the original and the export:", float(np.abs(expected - got).max()))
