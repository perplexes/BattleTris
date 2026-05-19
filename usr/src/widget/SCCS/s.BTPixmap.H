h29007
s 00000/00000/00000
d R 1.2 01/10/20 13:35:18 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTPixmap.H
c Name history : 1 0 src/widget/BTPixmap.H
e
s 00035/00000/00000
d D 1.1 01/10/20 13:35:17 bmc 1 0
c date and time created 01/10/20 13:35:17 by bmc
e
u
U
f e 0
t
T
I 1
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
E 1
