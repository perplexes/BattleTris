#include "BTConfig.H"

#include <string.h>

#include "btcmds.H"
#include "btref.H"

char nlisthelp[] =	"Print short listing of the network database";
char ndatahelp[] =	"Print all data in network database";
char ndeletehelp[] =	"Delete an entry from the network database";
char nflushhelp[] =	"Flush the entire network database file";
char ncrufthelp[] =	"Display possible cruft in network database";
char ncleanhelp[] =	"Clean cruft entries from network database";
char ncompresshelp[] =	"Compress network database file";
char plisthelp[] =	"Print short listing of the player database";
char pdatahelp[] =	"Print all data in player database";
char pdeletehelp[] =	"Delete player entries matching a pattern";
char pflushhelp[] =	"Flush the entries player database file";
char pcompresshelp[] =	"Compress player database file";
char statshelp[] =	"Report statistics about player database";
char helphelp[] =	"Display list of Referee commands";
char quithelp[] =	"Quit the BattleTris Referee program";

Command cmdtab[] = {
   { "nlist",		nlisthelp,	cmd_nlist },
   { "ndata",		ndatahelp,	cmd_ndata },
   { "ndelete",		ndeletehelp,	cmd_ndelete },
   { "nflush",		nflushhelp,	cmd_nflush },
   { "ncruft",		ncrufthelp,	cmd_ncruft },
   { "nclean",		ncleanhelp,	cmd_nclean },
   { "ncompress",	ncompresshelp,	cmd_ncompress },
   { "plist",		plisthelp,	cmd_plist },
   { "pdata",		pdatahelp,	cmd_pdata },
   { "pdelete",		pdeletehelp,	cmd_pdelete },
   { "pflush",		pflushhelp,	cmd_pflush },
   { "pcompress",	pcompresshelp,	cmd_pcompress },
   { "stats",		statshelp,	cmd_stats },
   { "help",		helphelp,	cmd_help },
   { "quit",		quithelp,	cmd_quit },
   { 0 }
};

int CMDTABLEN = (sizeof(cmdtab) / sizeof(cmdtab[0])) - 1;
int CMDMAXLEN = strlen("ncompress");
