# panel.md — Indian Railways panel indications

Written 2026-11-18 for `PLAN-schematic.md` slice S0. Sources fetched with
`obscura fetch <url> --dump text`; the 2023 PDF returned the site's HTML
"Site Under Construction" page on the normal fetch and again on the one
`--stealth` retry. The SEM PDF timed out on the normal fetch and the one
`--stealth` retry returned PDF bytes rather than extracted text. The text
below was checked against the official Indian Railways PDFs.

| ID | Claim | Source | Conf |
|---|---|---|---|
| PN-01 | Clear track is unlit: “no light when the line is unoccupied.” | Indian Railways, *Chapter 17: Train Detection — Track Circuits & Axle Counters* (2023), §17.2.6(a)(i), [PDF](https://indianrailways.gov.in/railwayboard/uploads/directorate/signal/2023/17-Train%20Detection%20-%20Track%20Circuits%20%26%20Axle%20Counters.pdf) | primary |
| PN-02 | A set, unoccupied route is yellow or white: “shows Yellow/White, if unoccupied.” | Indian Railways, *Chapter 17: Train Detection — Track Circuits & Axle Counters* (2023), §17.2.6(a)(i), [PDF](https://indianrailways.gov.in/railwayboard/uploads/directorate/signal/2023/17-Train%20Detection%20-%20Track%20Circuits%20%26%20Axle%20Counters.pdf) | primary |
| PN-03 | Occupied track is red regardless of route state: “a ‘RED’ indication ... irrespective of the route being set or not.” | Indian Railways, *Chapter 17: Train Detection — Track Circuits & Axle Counters* (2023), §17.2.6(a)(i), [PDF](https://indianrailways.gov.in/railwayboard/uploads/directorate/signal/2023/17-Train%20Detection%20-%20Track%20Circuits%20%26%20Axle%20Counters.pdf) | primary |
| PN-04 | Point normal indication colour and exact rendering | assumed | assumed |
| PN-05 | Point reverse indication colour and exact rendering | assumed | assumed |
| PN-06 | Signal ON indication colour and exact rendering | assumed | assumed |
| PN-07 | Control panel should indicate point position and signal aspect: “The position of points and aspects of Signals, condition of routes, as required.” | Indian Railways, *Signal Engineering Manual, Part I*, §7.172.2(i), [PDF](https://rb.indianrailways.gov.in/uploads/codesmanual/SEM-I/Sem_1_chap-VII.pdf) | primary |
| PN-08 | Set and locked route indication is white: “White indication to indicate that the route has been set and locked.” | Indian Railways, *Signal Engineering Manual, Part I*, §7.172.2(iv), [PDF](https://rb.indianrailways.gov.in/uploads/codesmanual/SEM-I/Sem_1_chap-VII.pdf) | primary |
| PN-09 | The TTC Transit Control Centre wall (Toronto): clear track lit pale blue, long runs green, short sections red, some runs magenta, trains as small white boxes, station names white with a small green or red time under each; a green seven-segment clock and a red and amber dot-matrix board above. Sampled from the photo: track `#a8c0f0`, green `#90c078`, red `#783030` (camera-washed). No published colour key was found; green as route set and red as occupied are read from where they lie | Your photo, shared 2026-11-19; three web searches found no key | photo, unsourced key |

The 2023 chapter says yellow/white for a set route, while SEM §7.172 specifies
white for a set-and-locked route. S3 used white, matching the specific
set-and-locked indication; since 2026-11-19 the game draws PN-09's look
instead, at your choice, and keeps PN-03's red. The sources do not assign colours to the point
lies or to a signal at ON.
