/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTBox.C                                             */
/*    ASSN:                                                     */
/*    DATE: Sun Feb  6 15:18:47 1994                            */
/****************************************************************/

#include "BTConfig.H"

#include <assert.h>

#include "BTBox.H"
#include "BTGame.H"
#include "BTPixmap.H"
#include "BTDisplay.H"
#include "BTXPalette.H"
#include "BTWidget.H"

#define BT_HAP_X1 2
#define BT_HAP_X2 3
#define BT_HAP_X3 11
#define BT_HAP_Y1 1
#define BT_HAP_Y2 8
#define BT_HAP_Y3 13
#define BT_HAP_XRAD 4
#define BT_HAP_YRAD 7
#define BT_HAP_XRAD2 11
#define BT_HAP_YRAD2 5

BTPixmap *box_maps[BT_MAX_BOXES];
int pixmaps_initialized_ = 0;
GC black_gc;

BTBoxManager::BTBoxManager(BTWidget *da, BTPixmap *gimp) {

  display_ = da ? XtDisplay(*da) : 0;

  if (!display_)  // Must be Ernie
    return;

  window_ = XtWindow(da->getWidget());
  screen_ = XtScreen(da->getWidget());
  colormap_ = DefaultColormap (display_, 0);

  // No need for Recon to create pixmaps again
  if (pixmaps_initialized_)
    return;
  pixmaps_initialized_ = 1;
  for ( int j = 0; j < BT_MAX_BOXES ; j++ )
    box_maps[j] = 0;

  gc_palette_.resize(BT_MAX_COLORS);

  XGCValues gc_values;
  unsigned long mask = 0;

  int i;
  // Run through the colors, setting up a graphics context for each one.
  for ( i = 0; i < BT_MAX_COLORS; i++) {
    gc_palette_[i] = XCreateGC (display_, window_, mask, &gc_values);
    BTXColor *btcolor = ((BTXPalette *)(DISPLAY->palette_))->getColor(i);
    XColor color = btcolor->color_;
//    XSetForeground (display, gc_palette_[i], (*p)[i].pixel);
    XSetForeground (display_, gc_palette_[i], color.pixel);
    XSetBackground (display_, gc_palette_[i], color.pixel);
  }
  black_gc = gc_palette_[BT_BLACK];

  for ( i = 0 ; i < BT_NEUTRAL ; i++ ) {
    box_maps[i] = new BTPixmap( BT_BOX_WTH, BT_BOX_HGT );
    XFillRectangle (display_, *box_maps[i], gc_palette_[i+BT_MAX_DIF_COLORS],
		    0, 0, BT_BOX_WTH, BT_BOX_HGT);
    XFillRectangle (display_, *box_maps[i], gc_palette_[i], 0, 0, 
		    BT_BOX_WTH - BT_BOX_BRDR, BT_BOX_HGT - BT_BOX_BRDR);
  }
  // Create the neutral boxes
  box_maps[BT_STRUCT] = new BTPixmap( BT_BOX_WTH, BT_BOX_HGT );
  XFillRectangle (display_, *box_maps[BT_STRUCT], gc_palette_[BT_NEUTRAL],
		  0, 0, BT_BOX_WTH, BT_BOX_HGT);
  // Create the dice
  for ( i = 1; i <= 6 ; i++ ) {
    box_maps[i+BT_DIE_1-1] = new BTPixmap( BT_BOX_WTH, BT_BOX_HGT );
    XFillRectangle (display_, *box_maps[i+BT_DIE_1-1], gc_palette_[BT_GRAY],
		    0, 0, BT_BOX_WTH, BT_BOX_HGT);
    XFillRectangle (display_, *box_maps[i+BT_DIE_1-1], gc_palette_[BT_IVORY], 0, 0, 
		    BT_BOX_WTH - BT_BOX_BRDR, BT_BOX_HGT - BT_BOX_BRDR);
    if (i > 1) {
	XFillRectangle(display_, *box_maps[i+BT_DIE_1-1], gc_palette_[BT_GRAY], BT_DIE_X1,
		       BT_DIE_Y1, BT_DIE_RAD, BT_DIE_RAD);
	XFillRectangle(display_, *box_maps[i+BT_DIE_1-1], black_gc, BT_DIE_X1 + 1,
		       BT_DIE_Y1 + 1, BT_DIE_RAD - 2, BT_DIE_RAD - 2);
      }
      if (i > 3) {
	XFillRectangle(display_, *box_maps[i+BT_DIE_1-1], gc_palette_[BT_GRAY], BT_DIE_X3,
		       BT_DIE_Y1, BT_DIE_RAD, BT_DIE_RAD);
	XFillRectangle(display_, *box_maps[i+BT_DIE_1-1], black_gc, BT_DIE_X3 + 1,
		       BT_DIE_Y1 + 1, BT_DIE_RAD - 2, BT_DIE_RAD - 2);
      }
      if (i > 1) {
	XFillRectangle(display_, *box_maps[i+BT_DIE_1-1], gc_palette_[BT_GRAY], BT_DIE_X3,
		       BT_DIE_Y3, BT_DIE_RAD, BT_DIE_RAD);
	XFillRectangle(display_, *box_maps[i+BT_DIE_1-1], black_gc, BT_DIE_X3 + 1,
		       BT_DIE_Y3 + 1, BT_DIE_RAD - 2, BT_DIE_RAD - 2);
      }
      if (i > 3) {
	XFillRectangle(display_, *box_maps[i+BT_DIE_1-1], gc_palette_[BT_GRAY], BT_DIE_X1,
		       BT_DIE_Y3, BT_DIE_RAD, BT_DIE_RAD);
	XFillRectangle(display_, *box_maps[i+BT_DIE_1-1], black_gc, BT_DIE_X1 + 1,
		       BT_DIE_Y3 + 1, BT_DIE_RAD - 2, BT_DIE_RAD - 2);
      }
      if (i % 2 == 1) {
	XFillRectangle(display_, *box_maps[i+BT_DIE_1-1], gc_palette_[BT_GRAY], BT_DIE_X2,
		       BT_DIE_Y2, BT_DIE_RAD, BT_DIE_RAD);
	XFillRectangle(display_, *box_maps[i+BT_DIE_1-1], black_gc, BT_DIE_X2 + 1,
		       BT_DIE_Y2 + 1, BT_DIE_RAD - 2, BT_DIE_RAD - 2);
      }
      if (i == 6) {
	XFillRectangle(display_, *box_maps[i+BT_DIE_1-1], gc_palette_[BT_GRAY], BT_DIE_X1,
		       BT_DIE_Y2, BT_DIE_RAD, BT_DIE_RAD);
	XFillRectangle(display_, *box_maps[i+BT_DIE_1-1], black_gc, BT_DIE_X1 + 1,
		       BT_DIE_Y2 + 1, BT_DIE_RAD - 2, BT_DIE_RAD - 2);
	
	XFillRectangle(display_, *box_maps[i+BT_DIE_1-1], gc_palette_[BT_GRAY], BT_DIE_X3,
		       BT_DIE_Y2, BT_DIE_RAD, BT_DIE_RAD);
	XFillRectangle(display_, *box_maps[i+BT_DIE_1-1], black_gc, BT_DIE_X3 + 1,
		       BT_DIE_Y2 + 1, BT_DIE_RAD - 2, BT_DIE_RAD - 2);
	
      }
  }
  // Draw happy box
  box_maps[BT_HAPPY] = new BTPixmap( BT_BOX_WTH, BT_BOX_HGT );
  XFillRectangle (display_, *box_maps[BT_HAPPY], gc_palette_[BT_DYELLOW],
		  0, 0, BT_BOX_WTH, BT_BOX_HGT);
  XFillRectangle (display_, *box_maps[BT_HAPPY], gc_palette_[BT_YELLOW], 0, 0, 
		  BT_BOX_WTH - BT_BOX_BRDR, BT_BOX_HGT - BT_BOX_BRDR);
  XFillRectangle (display_, *box_maps[BT_HAPPY], gc_palette_[BT_DYELLOW], 0, 0,
		  BT_BOX_WTH, BT_BOX_HGT);
  XFillRectangle (display_, *box_maps[BT_HAPPY], gc_palette_[BT_YELLOW], 0, 0, 
		  BT_BOX_WTH - BT_BOX_BRDR, BT_BOX_HGT - BT_BOX_BRDR);
  XFillArc (display_, *box_maps[BT_HAPPY], black_gc, BT_HAP_X1, BT_HAP_Y1,
	    BT_HAP_XRAD, BT_HAP_YRAD, 0, 23040);
  XFillArc (display_, *box_maps[BT_HAPPY], black_gc, BT_HAP_X3, BT_HAP_Y1,
	    BT_HAP_XRAD, BT_HAP_YRAD, 0, 23040);
  XDrawArc (display_, *box_maps[BT_HAPPY], black_gc, BT_HAP_X2, BT_HAP_Y2,
	    BT_HAP_XRAD2, BT_HAP_YRAD2, 11520, 11520);

  // Draw unhappy box
  box_maps[BT_UNHAPPY] = new BTPixmap( BT_BOX_WTH, BT_BOX_HGT );
  XFillRectangle (display_, *box_maps[BT_UNHAPPY], gc_palette_[BT_DYELLOW],
		  0, 0, BT_BOX_WTH, BT_BOX_HGT);
  XFillRectangle (display_, *box_maps[BT_UNHAPPY], gc_palette_[BT_YELLOW], 0, 0, 
		  BT_BOX_WTH - BT_BOX_BRDR, BT_BOX_HGT - BT_BOX_BRDR);
  XFillRectangle (display_, *box_maps[BT_UNHAPPY], gc_palette_[BT_DYELLOW], 0, 0,
		  BT_BOX_WTH, BT_BOX_HGT);
  XFillRectangle (display_, *box_maps[BT_UNHAPPY], gc_palette_[BT_YELLOW], 0, 0, 
		  BT_BOX_WTH - BT_BOX_BRDR, BT_BOX_HGT - BT_BOX_BRDR);
  XFillArc (display_, *box_maps[BT_UNHAPPY], black_gc, BT_HAP_X1, BT_HAP_Y1,
	    BT_HAP_XRAD, BT_HAP_YRAD, 0, 23040);
  XFillArc (display_, *box_maps[BT_UNHAPPY], black_gc, BT_HAP_X3, BT_HAP_Y1,
	    BT_HAP_XRAD, BT_HAP_YRAD, 0, 23040);
  XDrawPoint (display_, *box_maps[BT_UNHAPPY], gc_palette_[BT_BLUE], BT_HAP_X3+1,BT_HAP_Y1+7);
  XDrawPoint (display_, *box_maps[BT_UNHAPPY], gc_palette_[BT_BLUE], BT_HAP_X3+1,BT_HAP_Y1+8);
  XDrawPoint (display_, *box_maps[BT_UNHAPPY], gc_palette_[BT_BLUE], BT_HAP_X3+2,BT_HAP_Y1+8);
  XFillArc (display_, *box_maps[BT_UNHAPPY], gc_palette_[BT_BLUE], BT_HAP_X3, BT_HAP_Y1+8,
	    3, 3, 0, 23040);
  XDrawArc (display_, *box_maps[BT_UNHAPPY], black_gc, BT_HAP_X2, BT_HAP_Y3,
	    BT_HAP_XRAD2, BT_HAP_YRAD2, 0, 11520);

  if ( gimp )
    box_maps[BT_GIMP_ID] = new BTPixmap( *gimp );
}

BTBoxManager::~BTBoxManager() {
  if ( display_ ) {
    int j;
    for ( j = 0; j < BT_MAX_BOXES ; j++ )
      if ( box_maps[j] ) {
	delete box_maps[j];
        box_maps[j] = 0;
      }
    // Be nice to X
    for ( j = 0; j < gc_palette_.size(); j++)
      XFreeGC(display_, gc_palette_[j]);
  }
}

BTBox *BTBoxManager::create (int x, int y, int color) {
  BTBox *new_box;
  if (!display_)
    // Ernie must want a box.
    new_box = new BTInvisiBox(color);
  else { 
    if (color == BT_INVISIBLE) {
      // Create an invisible box
      new_box = new BTInvisiBox(0);
    } else {
      // A neutral box is desired;  set the dropshadow to neutral as well
      new_box = new BTBox( display_, window_, box_maps[color], color );
    }
  }
  new_box->moveTo (x, y, 1, 1);
  return new_box;
}

BTBox *BTBoxManager::dieCreate (int x, int y, int value) {
  BTBox *new_box;
  if (!display_) 
    new_box = new BTDieBox (value);
  else 
    new_box = new BTDieBox (display_, window_, value);
  new_box->moveTo (x, y, 1, 1);
  return new_box;
}

BTBox *BTBoxManager::happyCreate (int x, int y, int landed) {
  BTBox *new_box;
  if (!display_)
    new_box = new BTHappyBox(landed);
  else 
    new_box = new BTHappyBox (display_, window_, landed);
  new_box->moveTo (x, y, 1, 1);
  return new_box;
}

BTBox *BTBoxManager::structureCreate (int x, int y) {
  BTBox *new_box;

  if ( display_ )
    new_box = new BTStructureBox(display_, window_);
  else
    new_box = new BTInvisiStructureBox();
  new_box->moveTo(x,y, 1, 1);
  return new_box;
}

BTBox *BTBoxManager::createGimp (int x, int y, int value) {
  BTBox *new_box;

  if ( display_ ) 
    new_box = new BTGimpBox (display_, window_, value);
  else {
    new_box = new BTGimpBox(value);
  }
  new_box->moveTo(x,y, 1, 1);
  return new_box;
}

// Given an ID for a box, creates it (used to pass boards around)
BTBox *BTBoxManager::createByID (int x, int y, int ID) {

  assert (id >= 0);

  if (ID == BT_STRUCT)
    return structureCreate(x,y);
  if ((ID >= BT_DIE_1) && (ID <= BT_DIE_6))
    return dieCreate (x, y, ID - BT_DIE_1 + 1);
  if (ID == BT_HAPPY)
    return happyCreate (x, y, 0);
  if (ID == BT_UNHAPPY)
    return happyCreate (x, y, 1);
  if (ID == BT_GIMP_ID)
    return createGimp(x, y);
  return create(x, y, ID);
}

void BTBoxManager::dispose (BTBox *old) {
  delete old;
}

void BTBox::moveTo(int x, int y, int no_erase, int no_redraw) {
  if (display_ && (x_ >= 0) && (y_ >= 0) && !no_erase && (x < BT_BOARD_WTH)
      && (y <= BT_BOARD_HGT))
    erase();
  x_ = x * BT_BOX_WTH;  
  y_ = y * BT_BOX_HGT;
  if ( no_redraw == 0 )
    redraw();
}

void BTBox::redraw() {
  if ( ! display_ )
    return;
  // if we\'re hidden, don\'t draw color
  if ( hidden_ ) {
    erase();
    return;
  }
  if ( pixmap_ )
    XCopyArea( display_, *pixmap_, window_, black_gc,
	       0, 0, BT_BOX_WTH, BT_BOX_HGT, x_, y_ );
}

void BTBox::erase() {
  if ( ! display_ )
    return;
  XFillRectangle (display_, window_, black_gc, 
		  x_, y_, BT_BOX_WTH, BT_BOX_HGT);
}

void BTHappyBox::landed() {
  landed_ = 1;
  pixmap_ = box_maps[BT_UNHAPPY];
  id_ = BT_UNHAPPY;
  if ( ! display_ )
    return;
  redraw();
}
