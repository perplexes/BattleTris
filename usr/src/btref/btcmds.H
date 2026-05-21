#ifndef BTCMDS_H
#define BTCMDS_H

#include "BTConfig.H"

#include <sys/types.h>

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
