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

BTDrawingAreaWidget::BTDrawingAreaWidget(BTWidget *parent, const char *name,
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

BTDrawingAreaWidget::BTDrawingAreaWidget(BTWidget *parent, const char *name,
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
    gc_ = XCreateGC(XtDisplay(me_), XtWindow(me_), GCForeground, &xgcvalues);
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

  XFlush(XtDisplay(me_));
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
