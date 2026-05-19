#ifndef BTREF_H
#define BTREF_H

#include "BTConfig.H"

#define BTREF_SUCCESS	0
#define BTREF_ERROR	1
#define BTREF_USAGE	2

#define BTREF_VERSION	"1.0"

void cmd_help(int argc, char **argv);

class BTConfigFile;

extern BTConfigFile *g_conf;

#endif
