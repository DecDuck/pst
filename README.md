# pst
A tiny pastebin daemon. Binds to two ports:
- `:9999`: raw TCP socket to upload files to
- `:3000`: HTTP server to fetch files once they're uploaded


## how to use

Use a command like:
```bash
cat file.txt | nc my.bin 9999
```

And `pst` will send you a HTTP url.

## how to set up