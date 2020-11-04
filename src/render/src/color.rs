#[derive(Clone)]
pub struct Color {
    pub red: f32, 
    pub green: f32,
    pub blue: f32,
    pub alpha: f32
}

impl Color {
    pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Color {
        Color {
            red,
            green,
            blue,
            alpha
        }
    }
}

/*impl From<Color> for String {
    fn from(color: Color) -> Self {
        let mut text_color = String::from("rgb(");
        text_color += &((color.red * 255_f32) as u8).to_string();
        text_color += &", ";
        text_color += &((color.green * 255_f32) as u8).to_string();
        text_color += &", ";
        text_color += &((color.blue * 255_f32) as u8).to_string();
        text_color += &")";
        text_color
    }
}*/

fn to_hex_char(color: f32) -> String {
    let r = (color * 255_f32) as u8;

    let res = format!("{:x}", r);
    if res.len() == 1 {
        format!("0{}", res)
    } else {
        res
    }
    
}

impl From<&Color> for String {
    fn from(color: &Color) -> Self {
        let red = to_hex_char(color.red);
        let green = to_hex_char(color.green);
        let blue = to_hex_char(color.blue);
        crate::log(&format!("{}", color.red));
        crate::log(&format!("{}", color.green));
        crate::log(&format!("{}", color.blue));
        let color = format!("#{}{}{}", red, green, blue);
        crate::log(&color);
        color
    }
}