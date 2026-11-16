init python:
    import math
    import random
    import json

image bg room = "bg/room.png"
image eileen happy = "eileen/happy.png"
define audio.theme = "audio/theme.ogg"

label start:
    play music "audio/theme.ogg"
    play sound "audio/click.wav"
    play voice "voice/v01.ogg"
    scene "bg/room.png"
    show "eileen/happy.png"
    movie "movies/intro.webm"
    return
