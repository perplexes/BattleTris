h36349
s 00000/00000/00000
d R 1.2 01/10/20 13:34:53 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/btref/btcmds.H
c Name history : 1 0 src/btref/btcmds.H
e
s 00035/00000/00000
d D 1.1 01/10/20 13:34:52 bmc 1 0
c CodeManager Uniquification: src/btref/btcmds.H
c date and time created 01/10/20 13:34:52 by bmc
e
u
U
f e 0
t
T
I 1
#ifndef BTCMDS_H
#define BTCMDS_H

#include "BTConfig.H"

#define BTCMDS_DELIM	"-------------------------\n"
#define BTCMDS_TMPDB	"btref.tmpdb"

typedef void (*handler_t)(int, char**);

struct Command {
   char *name_;
   char *help_;
   handler_t handler_;
};

void cmd_ndata(int argc, char **argv);
void cmd_nlist(int argc, char **argv);
void cmd_ndelete(int argc, char **argv);
void cmd_nflush(int argc, char **argv);
void cmd_ncruft(int argc, char **argv);
void cmd_nclean(int argc, char **argv);
void cmd_ncompress(int argc, char **argv);
void cmd_pdata(int argc, char **argv);
void cmd_plist(int argc, char **argv);
void cmd_pdelete(int argc, char **argv);
void cmd_pflush(int argc, char **argv);
void cmd_pcompress(int argc, char **argv);
void cmd_stats(int argc, char **argv);
void cmd_quit(int argc, char **argv);

static int livedaemon(pid_t& pid);
static int confirm();

#endif
E 1
