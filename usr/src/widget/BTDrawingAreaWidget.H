/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTDrawingArea.H                                     */
/*    ASGN: Final                                               */
/*    DATE: Fri Apr 22 00:40:04 1994                            */
/****************************************************************/

#ifndef BTDRAWINGAREAWIDGET_H
#define BTDRAWINGAREAWIDGET_H

#include "BTConfig.H"

#include <Xm/DrawingA.h>

#include "BTWidget.H"

class BTPixmap;

class BTDrawingAreaWidget : public BTWidget {

public:
  ActivateCBStruct *expose_struct_, *input_struct_;
  BTPixmap *pixmap_;
  GC gc_;
  Dimension width_, height_;
  Boolean first_time_, button_released_;
  Dimension shape_mask_width_, shape_mask_height_;
  char *shape_mask_bits_;
  Pixmap shape_pixmap_;

  void exposeEvent();
  static void exposeEvent_CB(Widget widget, XtPointer data, XtPointer cbs) {
    BTDrawingAreaWidget *t = (BTDrawingAreaWidget *) data;
    t->exposeEvent();
  }
  virtual void addExposeCallback( void (*cb)(BTWidget *, void*), void *data ) {
    addCallback( expose_struct_, cb, data );
  }

  static void input_CB(Widget widget, XtPointer data, XtPointer cbs_pointer);

  virtual void addInputCallback( void (*cb)(BTWidget *, void*), void *data ) {
    addCallback( input_struct_, cb, data );
  }

  BTDrawingAreaWidget(BTWidget *parent, const char *name, BTPixmap *,
                      Dimension width, Dimension height,
                      Position x1, Position x2);
  BTDrawingAreaWidget(BTWidget *parent, const char *name, BTPixmap *,
                      Dimension width, Dimension height);
  ~BTDrawingAreaWidget();

  void setImage(BTPixmap *image);
  void setShape(char *bits, Dimension width, Dimension height);

/*
  static void _expose_ (Widget, XtPointer data, XEvent *, Boolean *) {
    ((BTWidget *)data)->expose();
  }
  virtual void expose() {
    if ( expose_struct_ )
      (*expose_struct_->cb_)(this, kb_struct_->data_);
  }
    if ( expose_struct_ ) {
//      delete expose_struct_;
//      XtRemoveRawEventHandler(me_, ExposureMask, FALSE, _expose_, this);
    }
    else
      expose_struct_ = new ActivateCBStruct();
    expose_struct_->cb_ = cb;
    expose_struct_->data_ = data;
//    XtAddRawEventHandler(me_, ExposureMask, FALSE, _expose_, this);
  */
};

#endif
