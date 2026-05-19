h41051
s 00002/00002/00160
d D 1.2 01/10/21 19:25:13 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:14 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTDrawingAreaWidget.C
c Name history : 1 0 src/widget/BTDrawingAreaWidget.C
e
s 00162/00000/00000
d D 1.1 01/10/20 13:35:13 bmc 1 0
c date and time created 01/10/20 13:35:13 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTDrawingArea.C                                     */
/*    ASGN: Final                                               */
/*    DATE: Fri Apr 22 00:41:12 1994                            */
/****************************************************************/

#include "BTConfig.H"

#include <Xm/Xm.h>
#include <assert.h>

#if HAVE_X11_EXTENSIONS_SHAPE_H
# include <X11/extensions/shape.h>
#endif

#include "BTDrawingAreaWidget.H"
#include "BattleTris.H"
#include "BTPixmap.H"

D 3
BTDrawingAreaWidget::BTDrawingAreaWidget(BTWidget *parent, char *const name,
E 3
I 3
BTDrawingAreaWidget::BTDrawingAreaWidget(BTWidget *parent, const char *name,
E 3
					 BTPixmap *pixmap,
                                         Dimension width, Dimension height,
					 Position x1, Position y1)
: BTWidget(parent), pixmap_(pixmap), gc_((GC) NULL),
  width_(width), height_(height), first_time_(True),
  shape_mask_width_(0), shape_mask_height_(0), shape_mask_bits_(NULL),
  shape_pixmap_((Pixmap) NULL), expose_struct_(0), input_struct_(0)
{
  Pixel fg, bg;
  int depth;

  me_ =
    XtVaCreateWidget(name, xmDrawingAreaWidgetClass, parent ? parent->getWidget() : 0,
                     XmNtopAttachment, XmATTACH_POSITION, XmNtopPosition, y1,
                     XmNleftAttachment, XmATTACH_POSITION, XmNleftPosition, x1,
                     XmNwidth, width, XmNheight, height,
		     XmNdepth, g_depth, NULL);

  XtAddCallback(me_, XmNexposeCallback, exposeEvent_CB, (XtPointer) this);
  XtAddCallback(me_, XmNinputCallback, input_CB, (XtPointer) this);

  // Add a reference to the pixmap
  if ( pixmap_ )
    pixmap_->ref();

/*
  if(image) {
    
    XtVaGetValues(me_, XmNforeground, &fg, XmNbackground, &bg,
                  XmNdepth, &depth, NULL);
    pixmap_ = XmGetPixmap(XtScreen(me_), image, fg, bg);
    XtVaSetValues(me_, XmNwidth, width, XmNheight, height, NULL);
  }
  */
}

D 3
BTDrawingAreaWidget::BTDrawingAreaWidget(BTWidget *parent, char *const name,
E 3
I 3
BTDrawingAreaWidget::BTDrawingAreaWidget(BTWidget *parent, const char *name,
E 3
                                         BTPixmap *pixmap,
                                         Dimension width, Dimension height)
: BTWidget(parent), pixmap_(pixmap), gc_((GC) NULL),
  width_(width), height_(height), first_time_(True),
  shape_mask_width_(0), shape_mask_height_(0), shape_mask_bits_(NULL),
  shape_pixmap_((Pixmap) NULL), expose_struct_(0), input_struct_(0)
{
  Pixel fg, bg;
  int depth;

  me_ = XtVaCreateWidget(name, xmDrawingAreaWidgetClass, parent->getWidget(),
                         XmNwidth, width, XmNheight, height, NULL);

  XtAddCallback(me_, XmNexposeCallback, exposeEvent_CB, (XtPointer) this);
  XtAddCallback(me_, XmNinputCallback, input_CB, (XtPointer) this);

  // Add a reference to the pixmap
  if ( pixmap_ )
    pixmap_->ref();
}

void BTDrawingAreaWidget::exposeEvent()
{
  XGCValues xgcvalues;

  assert( parent_ );

  if(first_time_) {
    xgcvalues.foreground = BlackPixelOfScreen(XtScreen(me_));
    gc_ = XCreateGC(XtDisplay(me_), XtWindow(parent_->getWidget()), GCForeground,&xgcvalues);
    first_time_ = False;
  }

#if HAVE_X11_EXTENSIONS_SHAPE_H
  if(shape_mask_bits_) {
    shape_pixmap_ =
      XCreateBitmapFromData(XtDisplay(parent_->getWidget()), XtWindow(parent_->getWidget()),
                            (const char *) shape_mask_bits_,
                            shape_mask_width_, shape_mask_height_);

    XShapeCombineMask(XtDisplay(parent_->getWidget()), XtWindow(parent_->getWidget()), ShapeBounding,
                      0, 0, shape_pixmap_, ShapeSet);
  }
#endif
  
  if(pixmap_)
    XCopyArea(XtDisplay(me_), *pixmap_, XtWindow(me_), gc_, 0, 0,
	      width_, height_, 0, 0);

  if ( expose_struct_ )
    (*expose_struct_->cb_)(this, expose_struct_->data_);
}

BTDrawingAreaWidget::~BTDrawingAreaWidget()
{
  if(shape_pixmap_)
    XFreePixmap(XtDisplay(parent_->getWidget()), shape_pixmap_);
  if(gc_)
    XFreeGC(XtDisplay(parent_->getWidget()), gc_);
  if ( pixmap_ )
    if ( pixmap_->deref() )
      delete pixmap_;
}

void BTDrawingAreaWidget::setShape(char *bits, Dimension width,Dimension height)
{
#if HAVE_X11_EXTENSIONS_SHAPE_H
  shape_mask_bits_ = bits;
  shape_mask_height_ = height; 
  shape_mask_width_ = width;
#endif
}

void BTDrawingAreaWidget::setImage(BTPixmap *image)
{
  XGCValues xgcvalues;
  Pixel fg, bg;
  int depth = 0;
    
  if(image) {
    if(pixmap_) {
      if ( pixmap_->deref() )
	delete pixmap_;
    }
    image->ref();
    pixmap_ = image;

    xgcvalues.foreground = BlackPixelOfScreen(XtScreen(me_));
    first_time_ = True;

    exposeEvent();
  }
}

void BTDrawingAreaWidget::input_CB(Widget widget, XtPointer data, XtPointer cbs_pointer) {
  BTDrawingAreaWidget *t = (BTDrawingAreaWidget *) data;
  XmDrawingAreaCallbackStruct *cbs = (XmDrawingAreaCallbackStruct *) cbs_pointer; 
  if ( cbs->event->xany.type == ButtonRelease)
    t->button_released_ = 1;
  else
    t->button_released_ = 0;
  t->callback( t->input_struct_ );
}
E 1
