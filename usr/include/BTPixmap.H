/****************************************************************/
/*    NAME:                                                     */
/*    ACCT: cgh                                                 */
/*    FILE: BTPixmap.H                                          */
/*    ASGN:                                                     */
/*    DATE: Tue Apr 18 13:28:46 1995                            */
/****************************************************************/

#ifndef BTPIXMAP_H
#define BTPIXMAP_H

#include "BTConfig.H"
#include <X11/Xlib.h>
#include <Xm/Xm.h>

class BTPixmap {

public:
  Pixmap pixmap_;
  unsigned short width_, height_;
  int ref_;

  BTPixmap(XImage *image, unsigned short width, unsigned short height,
	   int delete_ximage = 0);
  BTPixmap(unsigned short width, unsigned short height);
  BTPixmap(const BTPixmap& other);
  BTPixmap() : width_(0), height_(0), pixmap_(XmUNSPECIFIED_PIXMAP) {}
  BTPixmap& operator=(const BTPixmap& other);
  operator Pixmap() { return pixmap_; };
  void ref() { ref_++; }
  int deref() { return --ref_ == 0; }
  ~BTPixmap();
};

#endif
