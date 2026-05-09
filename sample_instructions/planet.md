I want you to create a sample geo-CLI. You can use Python, Rust or whatever language you want.
The program takes arguments: latitude and longitude. Then, the program will tell you which country the point belongs to.
If the location is ocean, it'll print "ocean".

It's CLI looks like this:

```
# latitude starts with "N" or "S", and followed by a number
# longitude starts with "E" or "W", and followed by a number
>>> geo --latitude=N37.42 --longitude=E127.11
korea

# This is point-nemo!
>>> geo --latitude=S48.8 --longitude=W123.4
ocean
```
