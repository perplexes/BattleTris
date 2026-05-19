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
