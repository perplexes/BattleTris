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
  unsigned char red_;
  unsigned char green_;
  unsigned char blue_;
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
  PPMReader(Widget widget, const char *root = 0);
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
