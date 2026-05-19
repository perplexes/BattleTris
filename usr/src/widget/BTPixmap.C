/****************************************************************/
/*    NAME:                                                     */
/*    ACCT: cgh                                                 */
/*    FILE: BTPixmap.C                                          */
/*    ASGN:                                                     */
/*    DATE: Tue Apr 18 13:32:57 1995                            */
/****************************************************************/

#include "BTConfig.H"

#include <stdio.h>

#include "BTPixmap.H"
#include "BTXDisplay.H"
#include "BattleTris.H"

BTPixmap::BTPixmap(XImage *image, unsigned short width, unsigned short height,
		   int delete_image)
: width_(width), height_(height), pixmap_(XmUNSPECIFIED_PIXMAP), ref_(0)
{
  pixmap_ = XCreatePixmap( g_display,
			   g_rootWindow,
			   width, height,
			   g_depth );

  XPutImage( g_display, pixmap_, g_GC,
	     image, 0, 0, 0, 0, width, height );

  if(delete_image) 
    XDestroyImage(image);
}

BTPixmap::BTPixmap(unsigned short width, unsigned short height)
: width_(width), height_(height), pixmap_(XmUNSPECIFIED_PIXMAP), ref_(0)
{
  pixmap_ = XCreatePixmap( g_display,
			   g_rootWindow,
			   width, height,
			   g_depth );
}

BTPixmap::BTPixmap(const BTPixmap &other)
: width_(other.width_), height_(other.height_),
  pixmap_(XmUNSPECIFIED_PIXMAP), ref_(0)
{
  pixmap_ = XCreatePixmap( g_display, g_rootWindow,
			   width_, height_,
			   g_depth );

  XCopyArea( g_display, other.pixmap_, pixmap_,
	     g_GC,
	     0, 0, width_, height_, 0, 0 );
}

BTPixmap& BTPixmap::operator=(const BTPixmap& other)
{
  if ( pixmap_ != XmUNSPECIFIED_PIXMAP )
    XFreePixmap(g_display, pixmap_);

  width_ = other.width_;
  height_ = other.height_;

  pixmap_ = XCreatePixmap( g_display, g_rootWindow,
			   width_, height_,
			   g_depth );

  XCopyArea( g_display, other.pixmap_, pixmap_,
	     g_GC,
	     0, 0, width_, height_, 0, 0 );

  return *this;
}

BTPixmap::~BTPixmap()
{
  if(pixmap_ != XmUNSPECIFIED_PIXMAP)
    XFreePixmap(g_display, pixmap_);
}
