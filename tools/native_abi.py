"""Shared ctypes contract for native runtime verification."""
import ctypes as C
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
U=C.c_uint32
F=C.c_float
P=C.c_void_p
S=C.c_char_p
class Draw(C.Structure):
    _fields_=[("kind",U),("rgba",U),("font",U),("align",U),("rect",F*4),("clip",F*4),("uv",F*4),("font_size",F),("rotation",F),("min_font_size",F),("reserved",U),("text",S),("xy",F*8)]
class Sound(C.Structure):
    _fields_=[("pcm",C.POINTER(F)),("frames",U),("reserved",U)]
class SoundCommand(C.Structure):
    _fields_=[("clip",U),("voice",U),("action",U),("gain",F)]
class Frame(C.Structure):
    _fields_=[("draws",C.POINTER(Draw)),("count",U),("wants_text",U),("width",F),("height",F),("input_rect",F*4),("audio_pcm",C.POINTER(F)),("audio_frames",U),("audio_epoch",U),("audio_state",U),("reserved",U),
              ("sounds",C.POINTER(Sound)),("sound_count",U),("sound_state",U),("sound_commands",C.POINTER(SoundCommand)),("sound_command_count",U),("sound_sequence",U)]
class Event(C.Structure):
    _fields_=[("kind",U),("key",U),("x",F),("y",F),("delta",F),("modifiers",U),("text",S)]
class API(C.Structure):
    _fields_=[("abi",U),("size",U),("name",S),("create",C.CFUNCTYPE(P,S,S)),
              ("destroy",C.CFUNCTYPE(None,P)),("font_path",C.CFUNCTYPE(S,P,U)),
              ("font_samples",C.CFUNCTYPE(S,P)),("frame",C.CFUNCTYPE(C.c_int,P,F,U,C.POINTER(Frame))),
              ("event",C.CFUNCTYPE(C.c_int,P,C.POINTER(Event))),
              ("take_request",C.CFUNCTYPE(U,P))]
assert C.sizeof(Draw)==120 and C.sizeof(Frame)==96 and C.sizeof(Event)==32
assert C.sizeof(Sound)==16 and C.sizeof(SoundCommand)==16
library=C.CDLL(str(ROOT/"target/release/libopensekai.so"))
library.asmui_application_get_api.argtypes=[U]
library.asmui_application_get_api.restype=C.POINTER(API)
assert not library.asmui_application_get_api(999)
assert not library.asmui_application_get_api(1)
assert not library.asmui_application_get_api(2)
assert not library.asmui_application_get_api(3)
assert not library.asmui_application_get_api(4)
api=library.asmui_application_get_api(5).contents
assert api.abi==5 and api.size==C.sizeof(API)
