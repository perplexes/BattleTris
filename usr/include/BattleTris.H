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
