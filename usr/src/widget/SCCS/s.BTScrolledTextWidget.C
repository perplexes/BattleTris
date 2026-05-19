h10303
s 00080/00076/00046
d D 1.2 01/10/21 19:25:18 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:17 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTScrolledTextWidget.C
c Name history : 1 0 src/widget/BTScrolledTextWidget.C
e
s 00122/00000/00000
d D 1.1 01/10/20 13:35:16 bmc 1 0
c date and time created 01/10/20 13:35:16 by bmc
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
/*    FILE: BTScrolledTextWidget.C                              */
/*    ASGN: Images                                              */
/*    DATE: Wed Apr 20 17:21:34 1994                            */
/****************************************************************/

#include "BTConfig.H"

#if STDC_HEADERS
# include <ctype.h>
#else
# define isspace(x) (((x) == ' ')  || ((x) == '\t'))
#endif

#include "BTScrolledTextWidget.H"

D 3
BTScrolledTextWidget::BTScrolledTextWidget(BTWidget *parent, char *const name,
                                           char *const text,
                                           Position x1, Position y1,
                                           Position x2, Position y2)
E 3
I 3
BTScrolledTextWidget::BTScrolledTextWidget(BTWidget *parent, const char *name,
    const char *text, Position x1, Position y1, Position x2, Position y2)
E 3
: BTWidget(parent)
{
D 3
  Arg args[20];
  int i = 0;
E 3
I 3
	Arg args[20];
	int i = 0;
E 3

D 3
  XtSetArg(args[i], XmNscrollingPolicy, XmAUTOMATIC); i++;
  XtSetArg(args[i], XmNscrollBarDisplayPolicy, XmSTATIC); i++;
  XtSetArg(args[i], XmNscrollBarPlacement, XmBOTTOM_RIGHT); i++;
  XtSetArg(args[i], XmNresizePolicy, XmRESIZE_NONE); i++;
  XtSetArg(args[i], XmNscrollHorizontal, False); i++;
  XtSetArg(args[i], XmNscrollVertical, True); i++;
E 3
I 3
	XtSetArg(args[i], XmNscrollingPolicy, XmAUTOMATIC); i++;
	XtSetArg(args[i], XmNscrollBarDisplayPolicy, XmSTATIC); i++;
	XtSetArg(args[i], XmNscrollBarPlacement, XmBOTTOM_RIGHT); i++;
	XtSetArg(args[i], XmNresizePolicy, XmRESIZE_NONE); i++;
	XtSetArg(args[i], XmNscrollHorizontal, False); i++;
	XtSetArg(args[i], XmNscrollVertical, True); i++;
E 3

D 3
  XtSetArg(args[i], XmNtopAttachment, XmATTACH_POSITION); i++;
  XtSetArg(args[i], XmNtopPosition, y1); i++;
  XtSetArg(args[i], XmNleftAttachment, XmATTACH_POSITION); i++;
  XtSetArg(args[i], XmNleftPosition, x1); i++;
  XtSetArg(args[i], XmNrightAttachment, XmATTACH_POSITION); i++;
  XtSetArg(args[i], XmNrightPosition, x2); i++;
  XtSetArg(args[i], XmNbottomAttachment, XmATTACH_POSITION); i++;
  XtSetArg(args[i], XmNbottomPosition, y2); i++;
  XtSetArg(args[i], XmNwidth, x2 - x1 + 1); i++;
  XtSetArg(args[i], XmNheight, y2 - y1 + 1); i++;
E 3
I 3
	XtSetArg(args[i], XmNtopAttachment, XmATTACH_POSITION); i++;
	XtSetArg(args[i], XmNtopPosition, y1); i++;
	XtSetArg(args[i], XmNleftAttachment, XmATTACH_POSITION); i++;
	XtSetArg(args[i], XmNleftPosition, x1); i++;
	XtSetArg(args[i], XmNrightAttachment, XmATTACH_POSITION); i++;
	XtSetArg(args[i], XmNrightPosition, x2); i++;
	XtSetArg(args[i], XmNbottomAttachment, XmATTACH_POSITION); i++;
	XtSetArg(args[i], XmNbottomPosition, y2); i++;
	XtSetArg(args[i], XmNwidth, x2 - x1 + 1); i++;
	XtSetArg(args[i], XmNheight, y2 - y1 + 1); i++;
E 3

D 3
  XtSetArg(args[i], XmNeditable, False); i++;
  XtSetArg(args[i], XmNeditMode, XmMULTI_LINE_EDIT); i++;
  XtSetArg(args[i], XmNcursorPositionVisible, False); i++;
  XtSetArg(args[i], XmNwordWrap, True); i++;
E 3
I 3
	XtSetArg(args[i], XmNeditable, False); i++;
	XtSetArg(args[i], XmNeditMode, XmMULTI_LINE_EDIT); i++;
	XtSetArg(args[i], XmNcursorPositionVisible, False); i++;
	XtSetArg(args[i], XmNwordWrap, True); i++;
E 3

D 3
  text_w_  = XmCreateScrolledText(parent ? parent->getWidget() : 0, name, args, i);
  XmTextSetString(text_w_, text);
  XtManageChild(text_w_);
E 3
I 3
	text_w_  = XmCreateScrolledText(parent ? parent->getWidget() : 0,
	    (char *)name, args, i);
	XmTextSetString(text_w_, (char *)text);
	XtManageChild(text_w_);
E 3

D 3
  me_ = XtParent(text_w_); // Get the scrolled window widget
E 3
I 3
	me_ = XtParent(text_w_); // Get the scrolled window widget
E 3
}

D 3
BTScrolledTextWidget::BTScrolledTextWidget(BTWidget *parent, char *const name,
                                           char *const text)
E 3
I 3
BTScrolledTextWidget::BTScrolledTextWidget(BTWidget *parent, const char *name,
    const char *text)
E 3
: BTWidget(parent)
{
D 3
  Arg args[10];
  int i = 0;
E 3
I 3
	Arg args[10];
	int i = 0;
E 3

D 3
  XtSetArg(args[i], XmNscrollingPolicy, XmAUTOMATIC); i++;
  XtSetArg(args[i], XmNscrollBarDisplayPolicy, XmSTATIC); i++;
  XtSetArg(args[i], XmNscrollBarPlacement, XmBOTTOM_RIGHT); i++;
  XtSetArg(args[i], XmNresizePolicy, XmRESIZE_NONE); i++;
  XtSetArg(args[i], XmNscrollHorizontal, False); i++;
  XtSetArg(args[i], XmNscrollVertical, True); i++;
E 3
I 3
	XtSetArg(args[i], XmNscrollingPolicy, XmAUTOMATIC); i++;
	XtSetArg(args[i], XmNscrollBarDisplayPolicy, XmSTATIC); i++;
	XtSetArg(args[i], XmNscrollBarPlacement, XmBOTTOM_RIGHT); i++;
	XtSetArg(args[i], XmNresizePolicy, XmRESIZE_NONE); i++;
	XtSetArg(args[i], XmNscrollHorizontal, False); i++;
	XtSetArg(args[i], XmNscrollVertical, True); i++;
E 3

D 3
  XtSetArg(args[i], XmNeditable, False); i++;
  XtSetArg(args[i], XmNeditMode, XmMULTI_LINE_EDIT); i++;
  XtSetArg(args[i], XmNcursorPositionVisible, False); i++;
  XtSetArg(args[i], XmNwordWrap, True); i++;
E 3
I 3
	XtSetArg(args[i], XmNeditable, False); i++;
	XtSetArg(args[i], XmNeditMode, XmMULTI_LINE_EDIT); i++;
	XtSetArg(args[i], XmNcursorPositionVisible, False); i++;
	XtSetArg(args[i], XmNwordWrap, True); i++;
E 3

D 3
  text_w_  = XmCreateScrolledText(parent ? parent->getWidget() : 0, name, args, i);
  XmTextSetString(text_w_, text);
  XtManageChild(text_w_);
E 3
I 3
	text_w_  = XmCreateScrolledText(parent ? parent->getWidget() : 0,
	    (char *)name, args, i);
	XmTextSetString(text_w_, (char *)text);
	XtManageChild(text_w_);
E 3

D 3
  me_ = XtParent(text_w_); // Get the scrolled window widget
E 3
I 3
	me_ = XtParent(text_w_); // Get the scrolled window widget
E 3
}

D 3
void BTScrolledTextWidget::splitLines(char *text, int width)
E 3
I 3
void
BTScrolledTextWidget::splitLines(char *text, int width)
E 3
{
D 3
  int last_space = 0, total = 0, line = 0;
E 3
I 3
	int last_space = 0, total = 0, line = 0;
E 3

D 3
  while(text[total]) {
    if(isspace(text[total]))
      last_space = total;
E 3
I 3
	while (text[total]) {
		if (isspace(text[total]))
			last_space = total;
E 3

D 3
    if(line == width) {
      text[last_space] = '\n';
      total = last_space;
    }
E 3
I 3
		if (line == width) {
			text[last_space] = '\n';
			total = last_space;
		}
E 3

D 3
    if(text[total] == '\n') {	
      line = 0;
      while(isspace(text[total]))
	total++;
    } else {
      line++;
      total++;
    }
  }
E 3
I 3
		if (text[total] == '\n') {	
			line = 0;
			while (isspace(text[total]))
				total++;
		} else {
			line++;
			total++;
		}
	}
E 3
}

D 3
void BTScrolledTextWidget::setText(char *const text, Boolean split, int width)
E 3
I 3
void
BTScrolledTextWidget::setText(const char *text, Boolean split, int width)
E 3
{
D 3
  char *desc = text; 
E 3
I 3
	char *desc; 
E 3

D 3
  if(split) {
    desc = (char *) XtMalloc(strlen(text) + 1);
    strcpy(desc, text);
    splitLines(desc, width);
  }
E 3
I 3
	if (split) {
		desc = (char *)XtMalloc(strlen(text) + 1);
		strcpy(desc, text);
		splitLines(desc, width);
  	} else {
		desc = (char *)text;
	}
E 3

D 3
  XmTextSetString(text_w_, desc);
E 3
I 3
	XmTextSetString(text_w_, desc);
E 3

D 3
  if(split)
    XtFree(desc);
E 3
I 3
	if (split)
		XtFree(desc);
E 3
}
E 1
