h09247
s 00001/00006/00059
d D 1.3 01/10/22 21:07:28 dep 4 3
c 1000014 off-by-one in ppm reader
e
s 00001/00001/00064
d D 1.2 01/10/21 19:25:13 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:35 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/PPMReader.H
c Name history : 1 0 src/game/PPMReader.H
e
s 00065/00000/00000
d D 1.1 01/10/20 13:35:34 bmc 1 0
c date and time created 01/10/20 13:35:34 by bmc
e
u
U
f e 0
t
T
I 1
#ifndef PPMREADER_H
#define PPMREADER_H

#include "BTConfig.H"

#include <X11/Xlib.h>
#include <X11/Intrinsic.h>
#include <stdio.h>

enum {
  PPM_MAX_RANGE		= 6,
  PPM_CHAR_MAX		= 255,
  PPM_USHRT_MAX		= 65535,
  PPM_INVALID_NUM	= -1
};

struct PPMPixel {
D 4

  // The order of these instance variables MUST go green, blue, red
  // so that it matches the order they are found in the PPM file so
  // we can just memcpy over this class --mws

E 4
I 4
  unsigned char red_;
E 4
  unsigned char green_;
  unsigned char blue_;
D 4
  unsigned char red_;
E 4
};

class PPMReader {

private: 
  char root_[1024];
  Widget widget_;

  unsigned short range_;
  short need_[PPM_CHAR_MAX];

  Pixel pixels_[PPM_CHAR_MAX];
  int num_colors_;
  XColor *color_cells_;
  Colormap cmap_;
  Visual *visual_;
  int depth_;

  int valid_;

private:
  int getNextNum(FILE* fp);

public:
D 3
  PPMReader(Widget widget, char *root = 0);
E 3
I 3
  PPMReader(Widget widget, const char *root = 0);
E 3
  ~PPMReader();

  int readPpmFile(const char * filename, PPMPixel *& data,
		  int& width, int& height);

  XImage *createImage(PPMPixel *data, const int width, const int height);

  operator int() { return valid_; }

private:
  PPMReader(const PPMReader&);
  PPMReader& operator=(const PPMReader&);
};

#endif
E 1
