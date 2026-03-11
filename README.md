# rhcu
Utility for compare two directories by hash-sum.
I vibecoded it using Cursor.

How it works:
```
rhcu -f "C:\First_dir" -s "C:\Secnd_dir"
Available arguments:
-a, --algo <ALGO> - hash-algorithm used in rhcu (<ALGO> is [default: blake3] [possible values: sha256, sha384, sha512, sha3-256, sha3-384, sha3-512, blake3])
-n, --no-color - disable colour
-f, --first <FIRST> (<FIRST> is Full path of first directory for compare in double quotation marks)
-s, --second <SECOND> (<SECOND> is Full path of first directory for compare in double quotation marks)
-h, --help - print this help
```
