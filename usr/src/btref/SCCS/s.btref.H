h42267
s 00000/00000/00000
d R 1.2 01/10/20 13:34:53 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/btref/btref.H
c Name history : 1 0 src/btref/btref.H
e
s 00018/00000/00000
d D 1.1 01/10/20 13:34:52 bmc 1 0
c date and time created 01/10/20 13:34:52 by bmc
e
u
U
f e 0
t
T
I 1
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
E 1
