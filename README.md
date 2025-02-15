# Discord Rich Presence for Jellyfin!
---

I wanted to have discord rich presence on Jellyfin but couldn't get anything else working, so I decided to make my own.
This is by no means a polished project or the most well-written one, but I think it'd be cool to share for anyone else who wants this specific niche.


## Usage
---
Run the provided binary with the command-line argument `config`, you will be asked to provide your Jellyfin API key, the ID of the discord application which you want to use (optional, it has a default), the username of the user whose activity you wish to track, and the **full** url of the jellyfin server (example: *http://jellyfin.johndoe.org* or *http://localhost:8096*).

When using it with a server hosted only on your local network (localhost), the Jellyfin server won't be able to provide URLs for images to be displayed in the rich presence.


