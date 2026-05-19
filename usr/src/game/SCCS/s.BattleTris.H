h29337
s 00000/00000/00000
d R 1.2 01/10/20 13:35:37 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BattleTris.H
c Name history : 1 0 src/game/BattleTris.H
e
s 00060/00000/00000
d D 1.1 01/10/20 13:35:36 bmc 1 0
c date and time created 01/10/20 13:35:36 by bmc
e
u
U
f e 0
t
T
I 1
#ifndef BATTLETRIS_H
#define BATTLETRIS_H

#include "BTConfig.H"

#include <Xm/Xm.h>

struct BTResources {
  Boolean headphones;           // Use headphones if available
  Boolean sleep;                // Startup in sleep mode
  Boolean r_rated;              // R-Rated mode
  Boolean a_team;               // Startup with A-Team theme
  Boolean mute;                 // Mute sound
  Boolean no_server;            // Do not connect to any server
  String serverHost;            // Server hostname
  int serverPort;               // Server port number
  XtTranslations keymappings;   // Translation table
  Pixel blackColor;             // BT_BLACK
  Pixel ivoryColor;             // BT_IVORY
  Pixel grayColor;              // BT_GRAY
  Pixel yellowColor;            // BT_YELLOW
  Pixel darkYellowColor;        // BT_DYELLOW
  Pixel blueColor;              // BT_BLUE
  Pixel darkBlueColor;          // BT_DBLUE
  Pixel neutralColor;           // BT_NEUTRAL
  Pixel redColor;               // BT_RED
  Pixel darkRedColor;           // BT_DRED
  Pixel orangeColor;            // BT_ORANGE
  Pixel darkOrangeColor;        // BT_DORANGE
  Pixel greenColor;             // BT_GREEN
  Pixel darkGreenColor;         // BT_DGREEN
  Pixel cyanColor;              // BT_CYAN
  Pixel darkCyanColor;          // BT_DCYAN
  Pixel purpleColor;            // BT_PURPLE
  Pixel darkPurpleColor;        // BT_DPURPLE
};

class BTStartup;

extern int g_signalfds[2];

extern XtAppContext g_appctx;
extern Display *g_display;
extern Widget g_toplevel;

extern Pixmap g_pixmap;
extern Colormap g_colormap;
extern int g_screen;
extern int g_depth;
extern GC g_GC;
extern Window g_rootWindow;
extern Visual *g_visual;

extern BTResources g_resources;
extern BTStartup *g_startup;


void bt_terminate(int retval);

#endif
E 1
