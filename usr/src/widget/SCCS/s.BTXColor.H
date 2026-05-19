h39771
s 00000/00000/00000
d R 1.2 01/10/20 13:35:19 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTXColor.H
c Name history : 1 0 src/widget/BTXColor.H
e
s 00042/00000/00000
d D 1.1 01/10/20 13:35:18 bmc 1 0
c date and time created 01/10/20 13:35:18 by bmc
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
/*    FILE: BTXColor.H                                          */
/*    ASGN:                                                     */
/*    DATE: Thu Sep 28 15:07:09 1995                            */
/****************************************************************/


#ifndef BTXCOLOR_HEADER
#define BTXCOLOR_HEADER

#include <X11/Xlib.h>
#include "BTColor.H"

class BTXColor : public BTColor {
public:
  BTXColor() {}
  BTXColor( XColor &color ) : color_(color) {}
  BTXColor( Pixel &pixel ) {
    color_.pixel = pixel;
    color_.red = (pixel & 0xFF0000);
    color_.green = (pixel & 0x00FF00);
    color_.blue = (pixel & 0x0000FF) << 8;
    color_.flags = DoRed | DoGreen | DoBlue;
  }
  BTXColor &operator=(Pixel &pixel) {
    color_.pixel = pixel;
    color_.red = (pixel & 0xFF0000) >> 8;
    color_.green = pixel & 0x00FF00;
    color_.blue = (pixel & 0x0000FF) << 8;
    color_.flags = DoRed | DoGreen | DoBlue;
    return *this;
  }
  BTXColor & operator=(XColor &color) { color_ = color; return *this;}
  operator XColor *() { return &color_; };

  XColor color_;

};

#endif
E 1
